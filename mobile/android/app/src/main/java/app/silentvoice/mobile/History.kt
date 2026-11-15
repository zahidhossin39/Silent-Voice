package app.silentvoice.mobile

import android.content.Context
import kotlinx.coroutines.flow.MutableStateFlow
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

data class Entry(
    val id: Long,
    val time: Long,
    val text: String,
    val audioSec: Float,
    val tookMs: Long,
    val mode: String?,
) {
    val words get() = text.split(Regex("\\s+")).count { it.isNotBlank() }
}

/** Past dictations, newest first, kept in one JSON file on the phone. */
object History {
    private const val MAX = 300
    private lateinit var file: File
    val items = MutableStateFlow<List<Entry>>(emptyList())

    fun init(ctx: Context) {
        file = File(ctx.filesDir, "history.json")
        items.value = runCatching {
            val arr = JSONArray(file.readText())
            List(arr.length()) { i ->
                val o = arr.getJSONObject(i)
                Entry(
                    o.getLong("id"), o.getLong("time"), o.getString("text"),
                    o.optDouble("audioSec", 0.0).toFloat(), o.optLong("tookMs"),
                    o.optString("mode").ifEmpty { null },
                )
            }
        }.getOrDefault(emptyList())
    }

    @Synchronized
    fun add(text: String, audioSec: Float, tookMs: Long, mode: String?) {
        val now = System.currentTimeMillis()
        items.value = (listOf(Entry(now, now, text, audioSec, tookMs, mode)) + items.value).take(MAX)
        save()
    }

    @Synchronized
    fun delete(id: Long) {
        items.value = items.value.filterNot { it.id == id }
        save()
    }

    @Synchronized
    fun clear() {
        items.value = emptyList()
        save()
    }

    private fun save() {
        val arr = JSONArray()
        items.value.forEach {
            arr.put(
                JSONObject().put("id", it.id).put("time", it.time).put("text", it.text)
                    .put("audioSec", it.audioSec.toDouble()).put("tookMs", it.tookMs).put("mode", it.mode ?: "")
            )
        }
        // Write beside, then swap: a crash mid-write can't leave half a file.
        val tmp = File(file.path + ".tmp")
        tmp.writeText(arr.toString())
        tmp.renameTo(file)
    }
}
