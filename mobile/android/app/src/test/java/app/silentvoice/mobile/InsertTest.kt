package app.silentvoice.mobile

import org.junit.Assert.assertEquals
import org.junit.Test

/** States captured from a real WhatsApp field on a Xiaomi 11i, 2026-11-19. */
class InsertTest {
    private val words = "I've been thinking. "

    @Test fun emptyWhatsAppFieldDoesNotKeepItsPlaceholder() {
        // Empty WhatsApp field: placeholder reported as text, not flagged, no cursor.
        val plan = planInsert("Message", showingHint = false, selStart = -1, selEnd = -1, add = words)
        assertEquals(InsertPlan.Paste, plan)
    }

    @Test fun realTextNamedMessageIsKept() {
        // The user really typed "Message": the app reports a cursor at its end.
        val plan = planInsert("Message", showingHint = false, selStart = 7, selEnd = 7, add = " hi")
        assertEquals(InsertPlan.SetText("Message hi", 10), plan)
    }

    @Test fun insertsAtTheCursor() {
        assertEquals(InsertPlan.SetText("hello big world", 9), planInsert("hello world", false, 5, 5, " big"))
    }

    @Test fun replacesTheSelection() {
        assertEquals(InsertPlan.SetText("hello there", 11), planInsert("hello world", false, 6, 11, "there"))
    }

    @Test fun flaggedHintCountsAsEmpty() {
        assertEquals(InsertPlan.SetText(words, words.length), planInsert("Type a message", true, -1, -1, words))
    }

    @Test fun trulyEmptyFieldIsSetDirectly() {
        assertEquals(InsertPlan.SetText(words, words.length), planInsert(null, false, -1, -1, words))
        assertEquals(InsertPlan.SetText(words, words.length), planInsert("", false, -1, -1, words))
    }

    @Test fun cursorOutsideTheTextIsNotTrusted() {
        assertEquals(InsertPlan.Paste, planInsert("abc", false, 9, 9, words))
    }
}
