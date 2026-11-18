package app.silentvoice.mobile

import java.net.URLDecoder
import java.net.URLEncoder

/**
 * The user's own words: a list the voice model should favour, and shortcuts
 * ("say this, type that"). Pure functions, so they're unit-tested on the JVM.
 */
object Words {
    /** How strongly listed words are favoured. Measured 2026-11-19 on Parakeet 110M TDT
     *  with spoken test clips: 1.5 and 2.0 fixed names (Zaid, Hossain, Kubernetes) and
     *  left normal sentences alone; 2.5 started writing "Claude" for "could". Same as desktop. */
    const val HOTWORDS_SCORE = 1.5f

    /**
     * The entries worth boosting, from a comma- or line-separated list. Names,
     * capitalised words, words with digits and phrases of several words are
     * kept. A single plain lowercase word is left out: the model already
     * spells everyday words, and boosting them makes it insert them into
     * unrelated speech ("could you call me" became "Claude you call me").
     * Desktop applies the same idea with a dictionary (sherpa_stt.rs).
     */
    fun boosted(list: String): List<String> {
        val out = mutableListOf<String>()
        for (raw in entries(list)) {
            // sherpa reads a word starting with ':' as a score (":2.0") and would
            // crash on ":D", so such words are dropped.
            val p = raw.split(Regex("\\s+")).filterNot { it.startsWith(':') }.joinToString(" ")
            if (p.isEmpty()) continue
            val words = p.split(Regex("[\\s-]+")).filter { it.isNotEmpty() }
            val special = words.size > 1 || p.any { it.isUpperCase() || it.isDigit() }
            if (special && out.none { it.equals(p, ignoreCase = true) }) out += p
        }
        return out
    }

    /** Every entry the user typed, trimmed, empties dropped. */
    fun entries(list: String): List<String> =
        list.split(',', '\n').map { it.trim() }.filter { it.isNotEmpty() }

    /** One shortcut. Whole words, not case-sensitive (see sv-core `apply_replacements`). */
    data class Shortcut(val say: String, val type: String)

    /**
     * Applies [rules] with [apply] (sv-core's `apply_replacements`). When the
     * dictation ends with a shortcut, the sentence-ending punctuation the voice
     * model put after it is dropped: "My email." must type the address, not
     * "name@example.com." A shortcut mid-sentence keeps normal punctuation.
     */
    fun expand(text: String, rules: List<Shortcut>, apply: (String) -> String): String {
        if (rules.isEmpty()) return text
        val end = Regex("[.!?]+\\s*$").find(text) ?: return apply(text)
        val core = text.substring(0, end.range.first)
        val endsWithShortcut = rules.any { endsWithWord(core, it.say.trim()) }
        return apply(if (endsWithShortcut) core else text)
    }

    /** True when [text] ends with [phrase] as whole words, ignoring case. */
    private fun endsWithWord(text: String, phrase: String): Boolean {
        if (phrase.isEmpty() || !text.endsWith(phrase, ignoreCase = true)) return false
        val before = text.length - phrase.length - 1
        return before < 0 || !text[before].isLetterOrDigit()
    }

    /** Stored one per line as "say<TAB>type", each part URL-encoded so tabs and line breaks survive. */
    fun encode(list: List<Shortcut>): String =
        list.joinToString("\n") { enc(it.say) + "\t" + enc(it.type) }

    fun decode(raw: String): List<Shortcut> =
        raw.lineSequence().mapNotNull { line ->
            val parts = line.split('\t')
            if (parts.size != 2) return@mapNotNull null
            val say = dec(parts[0]).trim()
            if (say.isEmpty()) null else Shortcut(say, dec(parts[1]))
        }.toList()

    private fun enc(s: String) = URLEncoder.encode(s, "UTF-8")
    private fun dec(s: String) = runCatching { URLDecoder.decode(s, "UTF-8") }.getOrDefault("")
}
