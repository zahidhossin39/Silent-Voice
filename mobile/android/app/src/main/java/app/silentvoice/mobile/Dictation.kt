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
    enum class Phase { Idle, Recording, Working }

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

    val model get() = MODELS.first { it.id == DEFAULT_MODEL_ID }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val stop = AtomicBoolean(false)
    private val loadLock = Mutex()
    @Volatile private var stt: Stt? = null
    private var idleJob: Job? = null
    private val threads = Runtime.getRuntime().availableProcessors().coerceIn(2, 4)

    fun modelReady(ctx: Context) = ModelStore.installed(ctx, model)

    fun micGranted(ctx: Context) =
        ctx.checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED

    /** Loads the model ahead of the first tap, so that tap doesn't pay for it. */
    fun warm(ctx: Context) {
        val app = ctx.applicationContext
        if (modelReady(app)) scope.launch { runCatching { engine(app) } }
    }

    private suspend fun engine(app: Context): Stt = loadLock.withLock {
        val loaded = stt ?: Stt.load(app, model, threads).also {
            stt = it
            Log.i(TAG, "model loaded in ${it.loadMs} ms")
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

    /** First call starts listening, second call finishes. [onDone] runs on the main thread. */
    fun toggle(ctx: Context, onDone: (Outcome) -> Unit) {
        when (phase.value) {
            Phase.Idle -> start(ctx.applicationContext, onDone)
            Phase.Recording -> stop.set(true)
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
        phase.value = Phase.Recording
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
        text = cleanAfterAi(text, opts).trim()
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
     */
    fun selfTest(ctx: Context) {
        val app = ctx.applicationContext
        scope.launch {
            try {
                val wav = ModelStore.sampleWav(app) ?: error("no sample clip (model not downloaded)")
                val (samples, rate) = readWav(wav)
                val r = engine(app).transcribe(samples, rate)
                val opts = CleanOptions(true, true, true, true, true)
                val text = cleanAfterAi(cleanBeforeAi(r.text, opts), opts).trim()
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

    fun start(ctx: Context) {
        if (progress.value != null) return
        val app = ctx.applicationContext
        progress.value = 0f
        error.value = null
        scope.launch {
            try {
                ModelStore.download(app, Dictation.model) { progress.value = it }
                Dictation.warm(app)
            } catch (e: Throwable) {
                Log.e(TAG, "model download failed", e)
                error.value = "The download stopped. Check your connection and try again."
            }
            progress.value = null
        }
    }
}
