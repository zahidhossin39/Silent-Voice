package app.silentvoice.mobile

import android.content.Context
import org.apache.commons.compress.archivers.tar.TarArchiveInputStream
import org.apache.commons.compress.compressors.bzip2.BZip2CompressorInputStream
import java.io.BufferedInputStream
import java.io.File
import java.net.HttpURLConnection
import java.net.URL

/** How a model's files map onto sherpa-onnx's config. */
enum class Engine { MOONSHINE, NEMO_CTC }

data class SttModel(val id: String, val label: String, val engine: Engine, val archive: String) {
    val url get() = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/$archive.tar.bz2"
}

/** Measured fastest on the test phone (mobile/android/README.md). */
const val DEFAULT_MODEL_ID = "parakeet-110m-en"

/** The three candidates that were measured; only the default is downloaded. */
val MODELS = listOf(
    SttModel("moonshine-tiny-en", "Moonshine Tiny", Engine.MOONSHINE, "sherpa-onnx-moonshine-tiny-en-int8"),
    SttModel("moonshine-base-en", "Moonshine Base", Engine.MOONSHINE, "sherpa-onnx-moonshine-base-en-int8"),
    SttModel("parakeet-110m-en", "Parakeet 110M", Engine.NEMO_CTC, "sherpa-onnx-nemo-parakeet_tdt_ctc_110m-en-36000-int8"),
)

object ModelStore {
    fun root(ctx: Context) = File(ctx.filesDir, "models")
    fun dir(ctx: Context, m: SttModel) = File(root(ctx), m.id)
    fun installed(ctx: Context, m: SttModel) = File(dir(ctx, m), "tokens.txt").exists()

    /**
     * Streams the archive straight into the unpacker, so the .tar.bz2 never sits
     * on disk. Unpacks into a temp folder and renames at the end: a half-finished
     * download can never look installed.
     */
    fun download(ctx: Context, m: SttModel, onProgress: (Float) -> Unit) {
        val tmp = File(root(ctx), "${m.id}.part").apply { deleteRecursively(); mkdirs() }
        val conn = URL(m.url).openConnection() as HttpURLConnection
        conn.instanceFollowRedirects = true
        conn.connect()
        check(conn.responseCode == 200) { "Download failed: HTTP ${conn.responseCode}" }
        val total = conn.contentLengthLong
        // Counting sits UNDER the buffer so it sees 64 KB blocks, not single bytes
        // (the bzip2 reader asks for one byte at a time), and it reports only on
        // each whole percent: a callback per byte made the download crawl.
        var lastPercent = -1
        val counting = CountingStream(conn.inputStream) { read ->
            val percent = if (total > 0) (read * 100 / total).toInt() else -1
            if (percent != lastPercent) { lastPercent = percent; onProgress(percent / 100f) }
        }
        TarArchiveInputStream(BZip2CompressorInputStream(BufferedInputStream(counting, 1 shl 16))).use { tar ->
            while (true) {
                val entry = tar.nextEntry ?: break
                // Drop the archive's top folder ("./name/" or "name/").
                val rel = entry.name.removePrefix("./").substringAfter('/', "")
                if (rel.isEmpty()) continue
                val out = File(tmp, rel)
                // An archive must never write outside its own folder.
                check(out.canonicalPath.startsWith(tmp.canonicalPath + File.separator)) { "Bad path in archive" }
                if (entry.isDirectory) out.mkdirs()
                else {
                    out.parentFile?.mkdirs()
                    out.outputStream().use { tar.copyTo(it) }
                }
            }
        }
        val dest = dir(ctx, m).apply { deleteRecursively() }
        check(tmp.renameTo(dest)) { "Could not move model into place" }
    }

    /** Frees space taken by models that were only downloaded for the speed test. */
    fun removeUnused(ctx: Context) {
        root(ctx).listFiles()?.forEach { if (it.name != DEFAULT_MODEL_ID) it.deleteRecursively() }
    }

    /** The longest test clip shipped with any installed model: the shared benchmark clip. */
    fun sampleWav(ctx: Context): File? =
        root(ctx).walkTopDown()
            .filter { it.isFile && it.extension == "wav" && it.parentFile?.name == "test_wavs" && it.name != "8k.wav" }
            .maxByOrNull { it.length() }
}

private class CountingStream(
    private val inner: java.io.InputStream,
    private val onRead: (Long) -> Unit,
) : java.io.InputStream() {
    private var count = 0L
    override fun read(): Int = inner.read().also { if (it >= 0) onRead(++count) }
    override fun read(b: ByteArray, off: Int, len: Int): Int =
        inner.read(b, off, len).also { if (it > 0) { count += it; onRead(count) } }
    override fun close() = inner.close()
}
