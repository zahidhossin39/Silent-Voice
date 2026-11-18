package app.silentvoice.mobile

import android.content.Context
import android.os.SystemClock
import com.k2fsa.sherpa.onnx.FeatureConfig
import com.k2fsa.sherpa.onnx.OfflineModelConfig
import com.k2fsa.sherpa.onnx.OfflineMoonshineModelConfig
import com.k2fsa.sherpa.onnx.OfflineNemoEncDecCtcModelConfig
import com.k2fsa.sherpa.onnx.OfflineRecognizer
import com.k2fsa.sherpa.onnx.OfflineRecognizerConfig
import com.k2fsa.sherpa.onnx.OfflineTransducerModelConfig
import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder

data class SttResult(val text: String, val audioSec: Float, val decodeMs: Long) {
    /** Real-time factor: time taken divided by audio length. Under 1 = faster than speech. */
    val rtf get() = if (audioSec > 0) decodeMs / 1000f / audioSec else 0f
}

/** One loaded model. Loading is the slow part, so keep it and reuse it. */
/**
 * Words to favour while decoding (sherpa "hotwords"). Needs a transducer model
 * and its SentencePiece [bpeVocab]; [key] tells a loaded engine whether the
 * list changed (the boost is fixed when the engine is built).
 */
class Hotwords(val bpeVocab: File, val file: File, val key: String)

class Stt private constructor(
    val model: SttModel,
    val loadMs: Long,
    private val rec: OfflineRecognizer,
    /** The word list this engine was built with ("" for none). */
    val hotwordsKey: String,
) {

    fun transcribe(samples: FloatArray, sampleRate: Int): SttResult {
        val t0 = SystemClock.elapsedRealtime()
        val stream = rec.createStream()
        stream.acceptWaveform(samples, sampleRate)
        rec.decode(stream)
        val text = rec.getResult(stream).text.trim()
        stream.release()
        return SttResult(text, samples.size.toFloat() / sampleRate, SystemClock.elapsedRealtime() - t0)
    }

    fun release() = rec.release()

    companion object {
        fun load(ctx: Context, m: SttModel, threads: Int, hot: Hotwords? = null): Stt {
            val d = ModelStore.dir(ctx, m)
            fun f(name: String) = File(d, name).absolutePath
            val modelConfig = when (m.engine) {
                Engine.MOONSHINE -> OfflineModelConfig(
                    moonshine = OfflineMoonshineModelConfig(
                        preprocessor = f("preprocess.onnx"),
                        encoder = f("encode.int8.onnx"),
                        uncachedDecoder = f("uncached_decode.int8.onnx"),
                        cachedDecoder = f("cached_decode.int8.onnx"),
                    ),
                    tokens = f("tokens.txt"),
                    numThreads = threads,
                )
                Engine.NEMO_TDT -> OfflineModelConfig(
                    transducer = OfflineTransducerModelConfig(
                        encoder = f("encoder.int8.onnx"),
                        decoder = f("decoder.int8.onnx"),
                        joiner = f("joiner.int8.onnx"),
                    ),
                    tokens = f("tokens.txt"),
                    numThreads = threads,
                    modelType = "nemo_transducer",
                )
                Engine.NEMO_CTC -> OfflineModelConfig(
                    nemo = OfflineNemoEncDecCtcModelConfig(model = f("model.int8.onnx")),
                    tokens = f("tokens.txt"),
                    numThreads = threads,
                )
            }
            val config = OfflineRecognizerConfig(
                featConfig = FeatureConfig(sampleRate = 16000, featureDim = 80),
                modelConfig = modelConfig,
            ).apply {
                // Greedy search has no alternatives to boost; beam search costs
                // ~0.1-0.2 s, so it's used only when there are words to favour.
                if (hot != null && m.engine == Engine.NEMO_TDT) {
                    // sherpa reads bpe.vocab only with modeling unit "bpe" and has no
                    // null check for it: set both together, only with a real file.
                    modelConfig.modelingUnit = "bpe"
                    modelConfig.bpeVocab = hot.bpeVocab.absolutePath
                    decodingMethod = "modified_beam_search"
                    hotwordsFile = hot.file.absolutePath
                    hotwordsScore = Words.HOTWORDS_SCORE
                }
            }
            val t0 = SystemClock.elapsedRealtime()
            // null asset manager = read model files from disk paths, not from the APK.
            val rec = OfflineRecognizer(null, config)
            val key = if (hot != null && m.engine == Engine.NEMO_TDT) hot.key else ""
            return Stt(m, SystemClock.elapsedRealtime() - t0, rec, key)
        }
    }
}

/** Reads a 16-bit PCM WAV file into floats in -1..1. Returns samples and sample rate. */
fun readWav(file: File): Pair<FloatArray, Int> {
    val buf = ByteBuffer.wrap(file.readBytes()).order(ByteOrder.LITTLE_ENDIAN)
    var sampleRate = 16000
    var channels = 1
    var pos = 12 // skip "RIFF", size, "WAVE"
    while (pos + 8 <= buf.capacity()) {
        val id = String(buf.array(), pos, 4)
        val size = buf.getInt(pos + 4)
        val body = pos + 8
        if (id == "fmt ") {
            channels = buf.getShort(body + 2).toInt()
            sampleRate = buf.getInt(body + 4)
        } else if (id == "data") {
            val frames = minOf(size, buf.capacity() - body) / 2 / channels
            val out = FloatArray(frames)
            for (i in 0 until frames) out[i] = buf.getShort(body + i * 2 * channels) / 32768f
            return out to sampleRate
        }
        pos = body + size + (size and 1) // chunks are padded to even length
    }
    error("No audio data in ${file.name}")
}
