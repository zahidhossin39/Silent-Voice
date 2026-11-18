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
