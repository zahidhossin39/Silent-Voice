package app.silentvoice.mobile

import android.Manifest
import android.annotation.SuppressLint
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.os.SystemClock
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.delay
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import uniffi.sv_core.CleanOptions
import uniffi.sv_core.Replacement
import uniffi.sv_core.applyReplacements
import uniffi.sv_core.cleanAfterAi
import uniffi.sv_core.cleanBeforeAi
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.math.sqrt

private const val TAG = "SVDictation"
private const val RATE = 16000
private const val MAX_SECONDS = 180
private const val IDLE_UNLOAD_MS = 5 * 60_000L

/**
 * The whole pipeline in one place, used by the bubble and by the in-app mic:
 * record, transcribe on the phone, clean up, optional writing mode, hand back text.
 * Any failure after transcription falls back to the plain words.
 */
object Dictation {
    /**
     * Starting: tapped, but the microphone isn't delivering audio yet (about
     * 200 ms on a Xiaomi 11i). The bubble keeps showing the mic until Recording,
     * so the listening look means the phone really is listening; speaking at
     * the first sight of it used to cut off the first syllable.
     */
    enum class Phase { Idle, Starting, Recording, Working }

    sealed interface Outcome {
        data class Text(val text: String, val notice: String? = null) : Outcome
        data class Problem(val message: String, val needsSetup: Boolean = false) : Outcome
    }

    val phase = MutableStateFlow(Phase.Idle)
    /**
     * Loudness of every 50 ms of audio, 0..1, mapped like the desktop pill (see
     * [Pill.norm]). A SharedFlow, not a StateFlow: silence is exactly 0 again and
     * again, and a StateFlow would drop the repeats, freezing the bars.
     */
    val level = MutableSharedFlow<Float>(extraBufferCapacity = 8, onBufferOverflow = BufferOverflow.DROP_OLDEST)

    /** The default model, or the old one while the default is still downloading. */
    fun model(ctx: Context): SttModel =
        if (!ModelStore.installed(ctx, DEFAULT_MODEL) && ModelStore.installed(ctx, LEGACY_MODEL)) LEGACY_MODEL
        else DEFAULT_MODEL

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val stop = AtomicBoolean(false)
    private val loadLock = Mutex()
    @Volatile private var stt: Stt? = null
    private var idleJob: Job? = null
    private val threads = Runtime.getRuntime().availableProcessors().coerceIn(2, 4)

    fun modelReady(ctx: Context) = ModelStore.installed(ctx, model(ctx))

    fun micGranted(ctx: Context) =
        ctx.checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED

    /** Loads the model ahead of the first tap, so that tap doesn't pay for it. */
    fun warm(ctx: Context) {
        val app = ctx.applicationContext
        if (modelReady(app)) scope.launch { runCatching { engine(app) } }
    }

    private suspend fun engine(app: Context): Stt = loadLock.withLock {
        val want = model(app)
        val hot = hotwords(app, want)
        // A finished upgrade download, or a changed word list, rebuilds the engine.
        stt?.takeIf { it.model != want || it.hotwordsKey != (hot?.key ?: "") }?.let { it.release(); stt = null }
        val loaded = stt ?: Stt.load(app, want, threads, hot).also {
            stt = it
            Log.i(TAG, "model ${want.id} loaded in ${it.loadMs} ms" + if (hot != null) " (favouring your words)" else "")
        }
        // The model holds a few hundred MB; let it go after a quiet spell.
        idleJob?.cancel()
        idleJob = scope.launch {
            delay(IDLE_UNLOAD_MS)
            loadLock.withLock {
                if (phase.value == Phase.Idle) {
                    stt?.release()
                    stt = null
                    Log.i(TAG, "model unloaded after idle")
                }
            }
        }
        loaded
    }

    /**
     * The user's words to favour, or null when there are none worth boosting or
     * the model can't be biased (only the TDT model can, and it needs its
     * tokenizer, shipped in the app's assets). Any problem means no boost,
     * never a failed dictation.
     */
    private fun hotwords(app: Context, m: SttModel): Hotwords? {
        if (m.engine != Engine.NEMO_TDT) return null
        val words = Words.boosted(Prefs.vocabulary)
        if (words.isEmpty()) return null
        return runCatching {
            val dir = ModelStore.dir(app, m)
            val vocab = java.io.File(dir, "bpe.vocab")
            // Written whole (temp file + rename) and replaced when it differs from the
            // app's copy: a file cut short by a kill would otherwise be trusted forever.
            val bytes = app.assets.open("${m.id}.bpe.vocab").use { it.readBytes() }
            if (vocab.length() != bytes.size.toLong()) {
                val tmp = java.io.File(dir, "bpe.vocab.tmp").apply { writeBytes(bytes) }
                check(tmp.renameTo(vocab)) { "could not place bpe.vocab" }
            }
            val list = words.joinToString("\n")
            val file = java.io.File(dir, "hotwords.txt").apply { writeText(list + "\n") }
            Hotwords(vocab, file, list)
        }.onFailure { Log.w(TAG, "word list not used: $it") }.getOrNull()
    }

    /** The user's "say this, type that" shortcuts (sv-core, same rules as desktop). */
    private fun shortcuts(text: String): String {
        val rules = Prefs.shortcuts.map { Replacement(it.say, it.type) }
        return if (rules.isEmpty()) text else applyReplacements(text, rules)
    }

    /** First call starts listening, second call finishes. [onDone] runs on the main thread. */
    fun toggle(ctx: Context, onDone: (Outcome) -> Unit) {
        when (phase.value) {
            Phase.Idle -> start(ctx.applicationContext, onDone)
            Phase.Starting, Phase.Recording -> stop.set(true)
            Phase.Working -> {}
        }
    }

    private fun start(app: Context, onDone: (Outcome) -> Unit) {
        if (!micGranted(app)) {
            onDone(Outcome.Problem("Microphone is off. Open Silent Voice to allow it.", needsSetup = true))
            return
        }
        if (!modelReady(app)) {
            onDone(Outcome.Problem("The voice model isn't downloaded yet. Open Silent Voice.", needsSetup = true))
            return
        }
        stop.set(false)
        phase.value = Phase.Starting
        scope.launch {
            val outcome = try {
                run(app)
            } catch (e: Throwable) {
                Log.e(TAG, "dictation failed", e)
                Outcome.Problem("Something went wrong. Nothing was typed.")
            }
            phase.value = Phase.Idle
            withContext(Dispatchers.Main) { onDone(outcome) }
        }
    }

    private suspend fun run(app: Context): Outcome {
        val loading = scope.async { engine(app) } // loads while the user is still talking
        val samples = record()
        val seconds = samples.size.toFloat() / RATE
        val loud = rms(samples, 0, samples.size)
        Log.i(TAG, "recorded ${"%.1f".format(seconds)} s, rms ${"%.4f".format(loud)}")
        if (seconds < 0.4f) return Outcome.Problem("Too quick. Tap, speak, then tap again.")
        if (loud < 0.0015f) return Outcome.Problem("I couldn't hear you. Is the microphone covered?")

        phase.value = Phase.Working
        val t0 = SystemClock.elapsedRealtime()
        val raw = loading.await().transcribe(samples, RATE).text
        if (raw.isBlank()) return Outcome.Problem("I didn't catch any words.")

        // Same steps and order as desktop; symbols and structure are always on there too.
        val opts = CleanOptions(Prefs.fillers, Prefs.repeats, true, true, Prefs.numbers)
        var text = cleanBeforeAi(raw, opts)
        var notice: String? = null
        var mode: String? = null
        if (Prefs.aiOn && Ai.configured()) {
            val chosen = Ai.current()
            runCatching { withContext(Dispatchers.IO) { Ai.rewrite(text, chosen) } }
                .onSuccess { text = it; mode = chosen.name }
                .onFailure {
                    Log.w(TAG, "writing mode failed: ${it.message}")
                    notice = "${chosen.name} mode didn't answer, so here are your exact words."
                }
        }
        // Shortcuts after any AI rewrite but before numbers become digits, the
        // desktop's order: a shortcut for "n eight n" must see "eight", not "8".
        text = cleanAfterAi(shortcuts(text), opts).trim()
        if (text.isEmpty()) return Outcome.Problem("I didn't catch any words.")
        val took = SystemClock.elapsedRealtime() - t0
        Log.i(TAG, "done in $took ms" + if (mode != null) " (with writing mode)" else "")
        History.add(text, seconds, took, mode)
        return Outcome.Text(text, notice)
    }

    /**
     * Runs the model's bundled sample clip through transcription and clean-up and
     * logs the result. Lets a build be checked from a PC without anyone speaking:
     *   adb shell am start -n app.silentvoice.mobile/.MainActivity --ez selftest true
     * [clip] names a WAV in the app's files/selftest folder instead (16 kHz mono),
     * so the user's word list and shortcuts can be checked with known speech.
     */
    fun selfTest(ctx: Context, clip: String? = null) {
        val app = ctx.applicationContext
        scope.launch {
            try {
                val wav = if (clip != null) java.io.File(app.filesDir, "selftest/$clip").takeIf { it.isFile } ?: error("no clip $clip")
                else ModelStore.sampleWav(app) ?: error("no sample clip (model not downloaded)")
                val (samples, rate) = readWav(wav)
                val r = engine(app).transcribe(samples, rate)
                val opts = CleanOptions(true, true, true, true, true)
                val text = cleanAfterAi(shortcuts(cleanBeforeAi(r.text, opts)), opts).trim()
                val probe = cleanAfterAi(cleanBeforeAi("Um, we need need twenty five.", opts), opts)
                Log.i(TAG, "SELFTEST ok: ${"%.1f".format(r.audioSec)} s audio in ${r.decodeMs} ms; clean-up probe=[$probe]; text=[$text]")
            } catch (e: Throwable) {
                Log.e(TAG, "SELFTEST failed: $e", e)
            }
        }
    }

    /** Records 16 kHz mono until [stop] is set or the time cap is reached. */
    @SuppressLint("MissingPermission") // start() checks RECORD_AUDIO first
    private fun record(): FloatArray {
        val minBuf = AudioRecord.getMinBufferSize(RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)
        val rec = AudioRecord(
            MediaRecorder.AudioSource.VOICE_RECOGNITION, RATE,
            AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT, minBuf * 2,
        )
        check(rec.state == AudioRecord.STATE_INITIALIZED) { "Microphone is busy" }
        val chunk = ShortArray(RATE / 20) // 50 ms
        var out = FloatArray(RATE * 30)
        var n = 0
        rec.startRecording()
        try {
            while (!stop.get() && n < RATE * MAX_SECONDS) {
                val got = rec.read(chunk, 0, chunk.size)
                if (got <= 0) continue
                if (n == 0) phase.value = Phase.Recording // audio is flowing: now show it
                if (n + got > out.size) out = out.copyOf(out.size * 2)
                for (i in 0 until got) out[n + i] = chunk[i] / 32768f
                level.tryEmit(Pill.norm(rms(out, n, n + got)))
                n += got
            }
        } finally {
            rec.stop()
            rec.release()
        }
        return out.copyOf(n)
    }


    private fun rms(a: FloatArray, from: Int, to: Int): Float {
        if (to <= from) return 0f
        var sum = 0.0
        for (i in from until to) sum += a[i] * a[i]
        return sqrt(sum / (to - from)).toFloat()
    }
}

/** Voice model download, shared by onboarding and Settings so both show the same progress. */
object ModelDownload {
    /** null = not running; 0..1 = progress. */
    val progress = MutableStateFlow<Float?>(null)
    val error = MutableStateFlow<String?>(null)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    /**
     * Phones that still have the old CTC model get the new one in the
     * background; dictation keeps using the old one until it lands.
     */
    fun upgradeIfNeeded(ctx: Context) {
        if (!upgradeAvailable(ctx)) return
        // 110 MB: only on Wi-Fi by itself; Settings offers it on mobile data.
        // Runs in App.onCreate: a failed check must never stop the app starting.
        val wifi = runCatching {
            !ctx.getSystemService(android.net.ConnectivityManager::class.java).isActiveNetworkMetered
        }.onFailure { Log.w(TAG, "network check failed: $it") }.getOrDefault(false)
        Log.i(TAG, "model upgrade: " + if (wifi) "downloading" else "waiting for Wi-Fi")
        if (wifi) start(ctx)
    }

    /** The old CTC model is in use and the better one isn't downloaded yet. */
    fun upgradeAvailable(ctx: Context) =
        !ModelStore.installed(ctx, DEFAULT_MODEL) && ModelStore.installed(ctx, LEGACY_MODEL)

    fun start(ctx: Context) {
        if (progress.value != null) return
        val app = ctx.applicationContext
        progress.value = 0f
        error.value = null
        scope.launch {
            try {
                ModelStore.download(app, DEFAULT_MODEL) { progress.value = it }
                ModelStore.removeUnused(app)
            } catch (e: Throwable) {
                Log.e(TAG, "model download failed", e)
                error.value = "The download stopped. Check your connection and try again."
            }
            progress.value = null
        }
    }
}
