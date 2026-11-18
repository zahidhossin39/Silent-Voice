package app.silentvoice.mobile

/** How to put dictated words into the focused field (see [planInsert]). */
sealed interface InsertPlan {
    /** Replace the field's text with [text] and put the cursor at [cursor]. */
    data class SetText(val text: String, val cursor: Int) : InsertPlan

    /** Paste at the app's own cursor. */
    data object Paste : InsertPlan
}

/**
 * Decides how to add [add] to a field whose accessibility node reports
 * [fieldText], [showingHint] and the selection [selStart]..[selEnd].
 *
 * The field's text is rewritten only when it is known to be real: the
 * framework says it is showing a hint (so it is empty), it is empty, or the
 * app reports a cursor inside it. Otherwise it may be a placeholder: an empty
 * WhatsApp field reports "Message" as its text, unflagged, with no cursor
 * (-1), and keeping it typed "MessageI've been...". Pasting then lets the app
 * itself put the words at its cursor, with the placeholder gone. Guessing
 * which text is a placeholder could wipe real words in other apps.
 */
fun planInsert(fieldText: String?, showingHint: Boolean, selStart: Int, selEnd: Int, add: String): InsertPlan {
    val current = if (showingHint) "" else fieldText.orEmpty()
    if (current.isEmpty()) return InsertPlan.SetText(add, add.length)
    val cursorKnown = selStart in 0..current.length && selEnd in 0..current.length
    if (!cursorKnown) return InsertPlan.Paste
    val from = minOf(selStart, selEnd)
    val to = maxOf(selStart, selEnd)
    return InsertPlan.SetText(current.substring(0, from) + add + current.substring(to), from + add.length)
}
