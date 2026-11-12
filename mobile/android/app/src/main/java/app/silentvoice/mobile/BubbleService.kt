package app.silentvoice.mobile

import android.accessibilityservice.AccessibilityService
import android.content.ClipData
import android.content.ClipboardManager
import android.graphics.PixelFormat
import android.graphics.Rect
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.Gravity
import android.view.View
import android.view.WindowManager
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityWindowInfo
import android.widget.ImageView
import android.widget.Toast

private const val TAG = "SVBubble"

/**
 * Bubble spike (docs/mobile/PLAN.md step 2). Shows a mic bubble just above the
 * keyboard whenever the keyboard is open, and on tap inserts a fixed test string
 * into the focused field. Real dictation replaces the test string in step 4.
 *
 * An accessibility service may draw its own overlay (TYPE_ACCESSIBILITY_OVERLAY),
 * so no separate "draw over other apps" permission is needed.
 *
 * What it reads: the list of windows (to find the keyboard) and, at the moment of
 * a tap, the focused field's text and cursor position (needed to insert at the
 * cursor). Nothing is stored or logged; password fields are never read or written.
 */
class BubbleService : AccessibilityService() {

    private val handler = Handler(Looper.getMainLooper())
    private val refresh = Runnable { update() }
    private var bubble: View? = null
    private var shownAtY = Int.MIN_VALUE

    override fun onServiceConnected() {
        Log.i(TAG, "connected")
        update()
    }

    // Events arrive in bursts while a keyboard animates in; wait for them to settle.
    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        handler.removeCallbacks(refresh)
        handler.postDelayed(refresh, 80)
    }

    override fun onInterrupt() {}

    override fun onDestroy() {
        handler.removeCallbacks(refresh)
        hide()
        super.onDestroy()
    }

    /** Top edge of the on-screen keyboard in screen pixels, or null when it is closed. */
    private fun keyboardTop(): Int? {
        val ime = windows.firstOrNull { it.type == AccessibilityWindowInfo.TYPE_INPUT_METHOD } ?: return null
        val r = Rect()
        ime.getBoundsInScreen(r)
        return if (r.height() > 0) r.top else null
    }

    private fun update() {
        val top = keyboardTop()
        if (top == null) hide() else show(top)
    }

    private fun dp(v: Int) = (v * resources.displayMetrics.density).toInt()

    private fun show(keyboardTop: Int) {
        val size = dp(52)
        val y = keyboardTop - size - dp(10)
        if (bubble != null && y == shownAtY) return
        val wm = getSystemService(WindowManager::class.java)
        val params = WindowManager.LayoutParams(
            size, size,
            WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
            // NOT_FOCUSABLE: tapping the bubble must not take focus (or the
            // keyboard) away from the field being typed in.
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
            PixelFormat.TRANSLUCENT,
        ).apply {
            gravity = Gravity.TOP or Gravity.END
            x = dp(12)
            this.y = y
            layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
        }
        val existing = bubble
        if (existing != null) {
            wm.updateViewLayout(existing, params)
        } else {
            val view = ImageView(this).apply {
                setImageResource(R.drawable.ic_mic)
                setPadding(dp(13), dp(13), dp(13), dp(13))
                background = GradientDrawable().apply {
                    shape = GradientDrawable.OVAL
                    setColor(0xFFF97316.toInt()) // Silent Voice orange
                }
                contentDescription = "Dictate with Silent Voice"
                setOnClickListener { insert(TEST_TEXT) }
            }
            wm.addView(view, params)
            bubble = view
            Log.i(TAG, "bubble shown")
        }
        shownAtY = y
    }

    private fun hide() {
        val view = bubble ?: return
        getSystemService(WindowManager::class.java).removeView(view)
        bubble = null
        shownAtY = Int.MIN_VALUE
        Log.i(TAG, "bubble hidden")
    }

    /**
     * Puts [text] at the cursor of the focused field. Order of attempts:
     * 1. set the field's text directly, 2. paste from the clipboard,
     * 3. leave it on the clipboard and say so. The words are never lost.
     */
    private fun insert(text: String) {
        val node = focusedField()
        if (node == null || !node.isEditable) {
            Log.i(TAG, "no field: node=${node != null} editable=${node?.isEditable} class=${node?.className} windows=" +
                windows.joinToString { "${it.type}/${it.isFocused}/${it.isActive}/root=${it.root != null}" })
            copy(text)
            done("no-field", "No text field found. Copied, paste it yourself.")
            return
        }
        if (node.isPassword) {
            done("password-skip", "Password field: nothing was typed.")
            return
        }
        // An empty field reports its grey hint as its text; treat that as empty.
        val current = if (node.isShowingHintText) "" else node.text?.toString().orEmpty()
        var start = node.textSelectionStart
        var end = node.textSelectionEnd
        if (start < 0 || end < 0 || start > current.length || end > current.length) {
            start = current.length
            end = current.length
        }
        val from = minOf(start, end)
        val to = maxOf(start, end)
        val newText = current.substring(0, from) + text + current.substring(to)
        val set = node.performAction(
            AccessibilityNodeInfo.ACTION_SET_TEXT,
            Bundle().apply { putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, newText) },
        )
        if (set) {
            val cursor = from + text.length
            node.performAction(
                AccessibilityNodeInfo.ACTION_SET_SELECTION,
                Bundle().apply {
                    putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, cursor)
                    putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, cursor)
                },
            )
            done("set-text", null)
            return
        }
        copy(text)
        if (node.performAction(AccessibilityNodeInfo.ACTION_PASTE)) done("paste", null)
        else done("clipboard-only", "This app blocked typing. Copied, paste it yourself.")
    }

    /**
     * The field with keyboard focus. The plain findFocus() only looks in the
     * "active" window, and touching the bubble makes the bubble that window, so
     * look through the app windows ourselves.
     */
    private fun focusedField(): AccessibilityNodeInfo? {
        val roots = windows
            .filter { it.type == AccessibilityWindowInfo.TYPE_APPLICATION }
            .sortedByDescending { it.isFocused }
            .mapNotNull { it.root }
        for (root in roots) {
            val focus = root.findFocus(AccessibilityNodeInfo.FOCUS_INPUT)
            if (focus != null) {
                // Some toolkits (Jetpack Compose) report a wrapper as focused,
                // with the real editable field inside it.
                return if (focus.isEditable) focus else focusedEditable(focus, 0) ?: focus
            }
        }
        return null
    }

    /**
     * The editable node at or below [node] that itself has focus. Never "the
     * first editable one": that typed into the wrong field when a password
     * field was focused. Looks at structure only, never text.
     */
    private fun focusedEditable(node: AccessibilityNodeInfo, depth: Int): AccessibilityNodeInfo? {
        if (node.isEditable && node.isFocused) return node
        if (depth > 12) return null
        for (i in 0 until node.childCount) {
            val child = node.getChild(i) ?: continue
            focusedEditable(child, depth + 1)?.let { return it }
        }
        return null
    }

    private fun copy(text: String) {
        getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("Silent Voice", text))
    }

    /** Logs only HOW the insert went, never the text. */
    private fun done(how: String, notice: String?) {
        Log.i(TAG, "insert: $how")
        if (notice != null) Toast.makeText(this, notice, Toast.LENGTH_SHORT).show()
    }

    companion object {
        const val TEST_TEXT = "Silent Voice test. "
    }
}
