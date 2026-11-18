package app.silentvoice.mobile

import app.silentvoice.mobile.Words.Shortcut
import org.junit.Assert.assertEquals
import org.junit.Test

class WordsTest {
    @Test fun keepsNamesJargonAndPhrases() {
        assertEquals(
            listOf("Kubernetes", "n8n", "Claude Code", "Zaid"),
            Words.boosted("Kubernetes, n8n, Claude Code\nZaid"),
        )
    }

    @Test fun skipsPlainLowercaseWords() {
        // Boosting everyday words makes the model insert them into normal speech.
        assertEquals(listOf("Gemini"), Words.boosted("task, ask, Gemini, discord"))
    }

    @Test fun dropsDuplicatesAndBlanks() {
        assertEquals(listOf("Tauri"), Words.boosted(" Tauri , ,tauri,\n\nTAURI"))
    }

    @Test fun dropsWordsSherpaWouldReadAsAScore() {
        // ":2.0" is sherpa's per-word score syntax; ":D" would crash its parser.
        assertEquals(listOf("Vue"), Words.boosted(":D, Vue :), :Big"))
    }

    // A stand-in for sv-core's apply_replacements: whole word, any case.
    private val rules = listOf(Shortcut("my email", "name@example.com"), Shortcut("brb", "be right back"))
    private fun fake(t: String) = t
        .replace(Regex("(?i)\\bmy email\\b"), "name@example.com")
        .replace(Regex("(?i)\\bbrb\\b"), "be right back")

    @Test fun shortcutAloneGetsNoFullStop() {
        assertEquals("name@example.com", Words.expand("My email.", rules, ::fake))
    }

    @Test fun shortcutEndingASentenceGetsNoFullStop() {
        assertEquals("Please send it to name@example.com", Words.expand("Please send it to my email.", rules, ::fake))
        assertEquals("be right back", Words.expand("BRB!", rules, ::fake))
    }

    @Test fun shortcutMidSentenceKeepsPunctuation() {
        assertEquals("name@example.com is best.", Words.expand("My email is best.", rules, ::fake))
    }

    @Test fun noShortcutLeavesTextAlone() {
        assertEquals("Hello there.", Words.expand("Hello there.", rules, ::fake))
        assertEquals("Hello there.", Words.expand("Hello there.", emptyList(), ::fake))
    }

    @Test fun partOfAWordIsNotAShortcut() {
        // "brb" inside "xbrb" is not the shortcut, so the full stop stays.
        assertEquals("Call xbrb.", Words.expand("Call xbrb.", rules, ::fake))
    }

    @Test fun emptyListBoostsNothing() {
        assertEquals(emptyList<String>(), Words.boosted("  \n , "))
    }

    @Test fun shortcutsSurviveTabsLineBreaksAndSymbols() {
        val list = listOf(
            Shortcut("my address", "12 Road\nDhaka"),
            Shortcut("n eight n", "n8n"),
            Shortcut("tab", "a\tb & c % d"),
        )
        assertEquals(list, Words.decode(Words.encode(list)))
    }

    @Test fun badStoredLinesAreSkipped() {
        // An empty "type" is allowed: a shortcut can delete a word.
        assertEquals(listOf(Shortcut("b", ""), Shortcut("a", "b")), Words.decode("garbage\n\ta\nb\t\na\tb"))
    }
}
