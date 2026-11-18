package app.silentvoice.mobile

import android.accessibilityservice.AccessibilityService
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.Canvas
import android.graphics.PixelFormat
import android.graphics.Rect
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import android.view.Gravity
import android.view.HapticFeedbackConstants
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import android.view.WindowInsets
import android.view.WindowManager
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityWindowInfo
import android.widget.Toast
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlin.math.hypot
import kotlin.math.roundToInt

private const val TAG = "SVBubble"

/**
 * The mic bubble. It shows whenever the keyboard is open, by the keyboard's
 * right edge until the user drags it somewhere else (remembered, and always kept
 * above the keyboard). Tap to start talking, tap again to finish, and the text
 * is typed at the cursor of the focused field.
 *
 * An accessibility service may draw its own overlay (TYPE_ACCESSIBILITY_OVERLAY),
 * so no separate "draw over other apps" permission is needed.
 *
 * What it reads: the list of windows (to find the keyboard) and, at the moment
 * text is inserted, the focused field's text and cursor position (needed to
 * insert at the cursor). Nothing is stored or logged; password fields are never
 * read or written.
 */
class BubbleService : AccessibilityService() {

    private val handler = Handler(Looper.getMainLooper())
    private val refresh = Runnable { update() }
    private val scope = MainScope()
    private var bubble: BubbleView? = null
    private var params: WindowManager.LayoutParams? = null
    private var shownAt = Int.MIN_VALUE to Int.MIN_VALUE
    private var lastKeyboardTop = 0
    private var dragging = false
    private var dragFrom = 0 to 0

    companion object {
        /** True while the system has this service bound (see [Keepalive.bubble]). */
        @Volatile
        var running = false
            private set
    }

    override fun onServiceConnected() {
        running = true
        Log.i(TAG, "connected")
        scope.launch {
            Dictation.phase.collect {
                bubble?.phase = it
                update() // a finished dictation may now let the bubble hide
            }
        }
        scope.launch { Dictation.level.collect { bubble?.push(it) } }
        update()
    }

    // Events arrive in bursts while a keyboard animates in; wait for them to settle.
    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        handler.removeCallbacks(refresh)
        handler.postDelayed(refresh, 80)
    }

    override fun onInterrupt() {}

    override fun onUnbind(intent: Intent?): Boolean {
        running = false
        return super.onUnbind(intent)
    }

    override fun onDestroy() {
        running = false
        handler.removeCallbacks(refresh)
        scope.cancel()
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
        when {
            top != null -> show(top)
            // Keyboard closed mid-dictation: stay put so the user can still finish.
            Dictation.phase.value != Dictation.Phase.Idle -> {}
            else -> hide()
        }
    }

    private fun dp(v: Int) = (v * resources.displayMetrics.density).toInt()

    /** The window's side: a 56 dp circle plus room for its shadow and the listening ring. */
    private fun size() = dp(56) + BubbleView.pad(this) * 2

    /**
     * Where the bubble's window may go: anywhere on screen below the status bar
     * and above the keyboard (never over the keys). The circle may reach the
     * screen edge; only its transparent padding goes past it.
     */
    private fun bounds(keyboardTop: Int): Rect {
        val wm = getSystemService(WindowManager::class.java)
        val metrics = wm.currentWindowMetrics
        val screen = metrics.bounds
        val top = metrics.windowInsets.getInsetsIgnoringVisibility(WindowInsets.Type.statusBars()).top
        val pad = BubbleView.pad(this)
        val size = size()
        val maxY = keyboardTop - size - (dp(8) - pad)
        val minY = minOf(top - pad, maxY)
        return Rect(-pad, minY, screen.width() - size + pad, maxY)
    }

    /** The bubble's spot: where the user dragged it, else 10 dp from the right edge, 8 dp above the keyboard. */
    private fun place(keyboardTop: Int): Pair<Int, Int> {
        val b = bounds(keyboardTop)
        val size = size()
        val screen = getSystemService(WindowManager::class.java).currentWindowMetrics.bounds
        val saved = Prefs.bubbleAt
        // b.right puts the circle's edge on the screen's edge.
        val x = if (saved == null) b.right - dp(10)
        else (saved.first * screen.width() - size / 2f).roundToInt()
        val y = if (saved == null) b.bottom else (saved.second * screen.height() - size / 2f).roundToInt()
        return x.coerceIn(b.left, b.right) to y.coerceIn(b.top, b.bottom)
    }

    private fun show(keyboardTop: Int) {
        lastKeyboardTop = keyboardTop
        if (dragging) return // the finger owns the position until it lifts
        val (x, y) = place(keyboardTop)
        val existing = bubble
        if (existing != null && x == shownAt.first && y == shownAt.second) return
        val wm = getSystemService(WindowManager::class.java)
        val size = size()
        val params = WindowManager.LayoutParams(
            size, size,
            WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
            // NOT_FOCUSABLE: tapping the bubble must not take focus (or the
            // keyboard) away from the field being typed in.
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
            PixelFormat.TRANSLUCENT,
        ).apply {
            gravity = Gravity.TOP or Gravity.START
            this.x = x
            this.y = y
            layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
        }
        if (existing != null) {
            wm.updateViewLayout(existing, params)
        } else {
            val view = BubbleView(this).apply {
                phase = Dictation.phase.value
                setOnClickListener { onTap(this) }
                // The model loads on touch, not when the keyboard opens: opening
                // a keyboard is common, dictating is not, and each load is a
                // couple of seconds of CPU. It finishes while the user talks.
                onDown = { Dictation.warm(context) }
                onDrag = ::drag
            }
            wm.addView(view, params)
            bubble = view
            Log.i(TAG, "bubble shown")
        }
        this.params = params
        shownAt = x to y
    }

    /** Follows the finger; [end] saves the spot. Offsets are from where the finger went down. */
    private fun drag(dx: Float, dy: Float, end: Boolean) {
        val view = bubble ?: return
        val p = params ?: return
        val b = bounds(lastKeyboardTop)
        if (!dragging) {
            dragging = true
            dragFrom = p.x to p.y
        }
        p.x = (dragFrom.first + dx).roundToInt().coerceIn(b.left, b.right)
        p.y = (dragFrom.second + dy).roundToInt().coerceIn(b.top, b.bottom)
        getSystemService(WindowManager::class.java).updateViewLayout(view, p)
        shownAt = p.x to p.y
        if (end) {
            dragging = false
            val screen = getSystemService(WindowManager::class.java).currentWindowMetrics.bounds
            val half = size() / 2f
            Prefs.bubbleAt = (p.x + half) / screen.width() to (p.y + half) / screen.height()
            Log.i(TAG, "bubble moved")
        }
    }

    private fun hide() {
        val view = bubble ?: return
        getSystemService(WindowManager::class.java).removeView(view)
        bubble = null
        params = null
        dragging = false
        shownAt = Int.MIN_VALUE to Int.MIN_VALUE
        Log.i(TAG, "bubble hidden")
    }

    private fun onTap(view: View) {
        view.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
        Dictation.toggle(this) { outcome ->
            when (outcome) {
                is Dictation.Outcome.Text -> {
                    bubble?.done()
                    insert(outcome.text + " ")
                    outcome.notice?.let { toast(it) }
                }
                is Dictation.Outcome.Problem -> {
                    toast(outcome.message)
                    if (outcome.needsSetup) {
                        startActivity(Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                    }
                }
            }
        }
    }

    /**
     * Puts [text] at the cursor of the focused field. Order of attempts:
     * 1. set the field's text directly, 2. paste from the clipboard,
     * 3. leave it on the clipboard and say so. The words are never lost.
     */
    private fun insert(text: String) {
        val node = focusedField()
        if (node == null || !node.isEditable) {
            copy(text)
            done("no-field", "Copied. Tap a text field and paste.")
            return
        }
        if (node.isPassword) {
            copy(text)
            done("password-skip", "That's a password field, so nothing was typed. Your words are copied.")
            return
        }
        when (val plan = planInsert(
            node.text?.toString(), node.isShowingHintText, node.textSelectionStart, node.textSelectionEnd, text,
        )) {
            is InsertPlan.SetText -> {
                val set = node.performAction(
                    AccessibilityNodeInfo.ACTION_SET_TEXT,
                    Bundle().apply { putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, plan.text) },
                )
                if (set) {
                    node.performAction(
                        AccessibilityNodeInfo.ACTION_SET_SELECTION,
                        Bundle().apply {
                            putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, plan.cursor)
                            putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, plan.cursor)
                        },
                    )
                    done("set-text", null)
                    return
                }
                paste(node, text, "paste")
            }
            // No cursor reported: the text may be a placeholder (see planInsert).
            InsertPlan.Paste -> paste(node, text, "paste-no-cursor")
        }
    }

    /** Pastes [text] at the app's own cursor; if the app refuses, leaves it on the clipboard and says so. */
    private fun paste(node: AccessibilityNodeInfo, text: String, how: String) {
        copy(text, trim = false) // keep the trailing space that separates it from the next words
        if (node.performAction(AccessibilityNodeInfo.ACTION_PASTE)) done(how, null)
        else done("clipboard-only", "This app blocked typing. Your words are copied, so just paste.")
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

    private fun copy(text: String, trim: Boolean = true) {
        val clip = if (trim) text.trim() else text
        getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("Silent Voice", clip))
    }

    private fun toast(message: String) = Toast.makeText(this, message, Toast.LENGTH_LONG).show()

    /** Logs only HOW the insert went, never the text. */
    private fun done(how: String, notice: String?) {
        Log.i(TAG, "insert: $how")
        if (notice != null) toast(notice)
    }
}

/**
 * The floating bubble above the keyboard ([Pill]: a mic while waiting, the
 * desktop pill's bars and an outer ring while listening), with a soft shadow
 * so it reads on any background.
 */
class BubbleView(context: Context) : View(context) {
    private val pill = Pill(resources.displayMetrics.density)

    var phase = Dictation.Phase.Idle
        set(value) {
            field = value
            pill.phase = value
            contentDescription = when (value) {
                Dictation.Phase.Idle -> "Start dictating with Silent Voice"
                Dictation.Phase.Starting -> "Starting the microphone"
                Dictation.Phase.Recording -> "Finish dictating"
                Dictation.Phase.Working -> "Silent Voice is writing"
            }
            invalidate()
        }

    /** Finger down: a tap or a drag is coming. */
    var onDown: (() -> Unit)? = null

    /** Finger moved past the touch slop: offsets in pixels since it went down; `end` on lift. */
    var onDrag: ((dx: Float, dy: Float, end: Boolean) -> Unit)? = null

    private val slop = ViewConfiguration.get(context).scaledTouchSlop
    private var downX = 0f
    private var downY = 0f
    private var dragging = false

    init {
        contentDescription = "Start dictating with Silent Voice"
    }

    // Raw (screen) coordinates: the window moves under the finger, so
    // view-relative ones would shake.
    @android.annotation.SuppressLint("ClickableViewAccessibility") // a tap still goes through performClick
    override fun onTouchEvent(e: MotionEvent): Boolean {
        when (e.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                downX = e.rawX
                downY = e.rawY
                dragging = false
                onDown?.invoke()
            }
            MotionEvent.ACTION_MOVE -> {
                val dx = e.rawX - downX
                val dy = e.rawY - downY
                if (!dragging && hypot(dx, dy) > slop) {
                    dragging = true
                    performHapticFeedback(HapticFeedbackConstants.CLOCK_TICK)
                }
                if (dragging) onDrag?.invoke(dx, dy, false)
            }
            MotionEvent.ACTION_UP -> {
                if (dragging) onDrag?.invoke(e.rawX - downX, e.rawY - downY, true) else performClick()
                dragging = false
            }
            MotionEvent.ACTION_CANCEL -> {
                if (dragging) onDrag?.invoke(e.rawX - downX, e.rawY - downY, true)
                dragging = false
            }
        }
        return true
    }

    fun push(level: Float) {
        pill.push(level)
        invalidate()
    }

    fun done() {
        pill.done()
        invalidate()
    }

    override fun onDraw(c: Canvas) {
        pill.draw(c, width.toFloat(), height.toFloat(), SystemClock.uptimeMillis(), pad(context).toFloat(), shadow = true)
        if (pill.animating) postInvalidateOnAnimation()
    }

    companion object {
        /** Space around the circle for its shadow and the listening ring (56 dp x [Pill.RING_ROOM] = 6.7 dp). */
        fun pad(ctx: Context) = (7 * ctx.resources.displayMetrics.density).toInt()
    }
}
