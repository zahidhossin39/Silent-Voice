package app.silentvoice.mobile

import android.Manifest
import android.annotation.SuppressLint
import android.content.pm.PackageManager
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.content.Intent
import android.os.Bundle
import android.provider.Settings
import android.text.InputType
import android.widget.EditText
import androidx.compose.ui.viewinterop.AndroidView
import android.util.Log
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.OutlinedTextField
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.util.concurrent.atomic.AtomicBoolean

private const val TAG = "SVBench"

/**
 * Engine spike (docs/mobile/PLAN.md step 1): record or use a sample clip, run one
 * of three small English models, show the text and how long it took.
 *
 * Headless run for measuring from a PC:
 *   adb shell am start -n app.silentvoice.mobile/.MainActivity --es bench all
 * Results are printed to logcat under the tag SVBench.
 */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val bench = intent.getStringExtra("bench")
        setContent { MaterialTheme { Surface(Modifier.fillMaxSize()) { SpikeScreen(bench) } } }
    }
}

@Composable
private fun SpikeScreen(bench: String?) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var selected by remember { mutableStateOf(MODELS.first()) }
    var stt by remember { mutableStateOf<Stt?>(null) }
    var status by remember { mutableStateOf("Pick a model.") }
    var progress by remember { mutableFloatStateOf(-1f) }
    var busy by remember { mutableStateOf(false) }
    var recording by remember { mutableStateOf(false) }
    var log by remember { mutableStateOf("") }
    val stop = remember { AtomicBoolean(false) }
    val threads = remember { Runtime.getRuntime().availableProcessors().coerceIn(2, 4) }

    fun report(line: String) {
        Log.i(TAG, line)
        log = line + "\n\n" + log
    }

    /** Download if needed, then load. Returns the loaded model. */
    suspend fun ensure(m: SttModel): Stt = withContext(Dispatchers.IO) {
        stt?.let { if (it.model == m) return@withContext it else it.release() }
        stt = null
        if (!ModelStore.installed(ctx, m)) {
            status = "Downloading ${m.label}..."
            ModelStore.download(ctx, m) { progress = it }
            progress = -1f
        }
        status = "Loading ${m.label}..."
        Stt.load(ctx, m, threads).also {
            stt = it
            status = "${m.label} ready (loaded in ${it.loadMs} ms, $threads threads)."
        }
    }

    suspend fun run(m: SttModel, samples: FloatArray, rate: Int, source: String) {
        val engine = ensure(m)
        val r = withContext(Dispatchers.Default) { engine.transcribe(samples, rate) }
        report(
            "${m.id} | $source | audio ${"%.1f".format(r.audioSec)} s | " +
                "decode ${r.decodeMs} ms | RTF ${"%.3f".format(r.rtf)} | load ${engine.loadMs} ms\n${r.text}"
        )
    }

    fun guarded(block: suspend () -> Unit) {
        if (busy) return
        busy = true
        scope.launch {
            try { block() } catch (e: Throwable) {
                progress = -1f
                status = "Failed: ${e.message}"
                Log.e(TAG, "failed", e)
            }
            busy = false
        }
    }

    suspend fun runSample(m: SttModel) {
        ensure(m)
        val wav = ModelStore.sampleWav(ctx) ?: error("No sample clip installed")
        val (samples, rate) = withContext(Dispatchers.IO) { readWav(wav) }
        run(m, samples, rate, "sample ${wav.name}")
    }

    val askMic = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        status = if (granted) "Microphone allowed. Tap Record." else "Microphone permission was refused."
    }

    // Headless benchmark: every model (or one id), sample clip, twice (cold then warm).
    LaunchedEffect(bench) {
        if (bench == null) return@LaunchedEffect
        guarded {
            val todo = if (bench == "all") MODELS else MODELS.filter { it.id == bench }
            for (m in todo) { runSample(m); runSample(m) }
            report("BENCH DONE")
        }
    }

    Column(
        Modifier.fillMaxSize().safeDrawingPadding().padding(16.dp).verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text("Silent Voice engine spike", style = MaterialTheme.typography.titleLarge)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            MODELS.forEach { m ->
                FilterChip(
                    selected = selected == m,
                    enabled = !busy && !recording,
                    onClick = { selected = m },
                    label = { Text(m.label) },
                )
            }
        }
        Text(status)
        if (progress >= 0f) LinearProgressIndicator(progress = { progress }, modifier = Modifier.fillMaxWidth())

        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                enabled = !busy || recording,
                onClick = {
                    if (recording) { stop.set(true); return@Button }
                    if (ctx.checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
                        askMic.launch(Manifest.permission.RECORD_AUDIO)
                        return@Button
                    }
                    guarded {
                        ensure(selected)
                        stop.set(false)
                        recording = true
                        status = "Recording... tap Stop."
                        val samples = withContext(Dispatchers.IO) { record(stop) }
                        recording = false
                        status = "Transcribing..."
                        run(selected, samples, 16000, "mic")
                        status = "${selected.label} ready."
                    }
                },
            ) { Text(if (recording) "Stop" else "Record") }
            OutlinedButton(enabled = !busy, onClick = { guarded { runSample(selected) } }) { Text("Run sample clip") }
        }
        Text(log, style = MaterialTheme.typography.bodySmall)

        // Bubble spike (plan step 2): turn the service on, then try it in these
        // two fields or in any other app.
        val bubbleOn = Settings.Secure.getString(ctx.contentResolver, Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES)
            .orEmpty().contains(ctx.packageName)
        Text("Bubble", style = MaterialTheme.typography.titleMedium)
        Text(if (bubbleOn) "Bubble service is on." else "Bubble service is off. Turn on \"Silent Voice bubble\" in Accessibility.")
        OutlinedButton(onClick = { ctx.startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS)) }) {
            Text("Open Accessibility settings")
        }
        var plain by remember { mutableStateOf("") }
        var secret by remember { mutableStateOf("") }
        // Native fields: what most apps (WhatsApp, Gmail) use.
        AndroidView(
            factory = { EditText(it).apply { hint = "Native test field" } },
            modifier = Modifier.fillMaxWidth(),
        )
        AndroidView(
            factory = {
                EditText(it).apply {
                    hint = "Native password field"
                    inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
                }
            },
            modifier = Modifier.fillMaxWidth(),
        )
        // Compose fields: the newer toolkit some apps use.
        OutlinedTextField(plain, { plain = it }, label = { Text("Test field") }, modifier = Modifier.fillMaxWidth())
        OutlinedTextField(
            secret, { secret = it },
            label = { Text("Password test field") },
            visualTransformation = PasswordVisualTransformation(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

/** Records 16 kHz mono from the microphone until [stop] is set. */
@SuppressLint("MissingPermission") // the caller checks RECORD_AUDIO first
private fun record(stop: AtomicBoolean): FloatArray {
    val rate = 16000
    val minBuf = AudioRecord.getMinBufferSize(rate, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)
    val rec = AudioRecord(
        MediaRecorder.AudioSource.VOICE_RECOGNITION, rate,
        AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT, minBuf * 2,
    )
    val chunk = ShortArray(rate / 10) // 100 ms
    var out = FloatArray(rate * 30)
    var n = 0
    rec.startRecording()
    try {
        while (!stop.get()) {
            val got = rec.read(chunk, 0, chunk.size)
            if (got <= 0) continue
            if (n + got > out.size) out = out.copyOf(out.size * 2)
            for (i in 0 until got) out[n + i] = chunk[i] / 32768f
            n += got
        }
    } finally {
        rec.stop()
        rec.release()
    }
    return out.copyOf(n)
}
