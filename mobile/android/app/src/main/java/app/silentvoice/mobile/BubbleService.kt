package app.silentvoice.mobile

import android.accessibilityservice.AccessibilityService
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.Rect
import android.graphics.RectF
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import android.view.Gravity
import android.view.HapticFeedbackConstants
import android.view.View
import android.view.WindowManager
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityWindowInfo
import android.widget.Toast
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch

private const val TAG = "SVBubble"

/**
 * The mic bubble. It sits just above the keyboard whenever the keyboard is open;
 * tap to start talking, tap again to finish, and the text is typed at the cursor
 * of the focused field.
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
    private var shownAtY = Int.MIN_VALUE

    override fun onServiceConnected() {
        Log.i(TAG, "connected")
        scope.launch {
            Dictation.phase.collect {
                bubble?.phase = it
                update() // a finished dictation may now let the bubble hide
            }
        }
        scope.launch { Dictation.level.collect { bubble?.level = it } }
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

    private fun show(keyboardTop: Int) {
        val size = dp(68)
        val y = keyboardTop - size - dp(4)
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
            x = dp(6)
            this.y = y
            layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
        }
        val existing = bubble
        if (existing != null) {
            wm.updateViewLayout(existing, params)
        } else {
            val view = BubbleView(this).apply {
                phase = Dictation.phase.value
                setOnClickListener { onTap(this) }
            }
            wm.addView(view, params)
            bubble = view
            Dictation.warm(this) // keyboard is up: a dictation is likely
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

    private fun onTap(view: View) {
        view.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
        Dictation.toggle(this) { outcome ->
            when (outcome) {
                is Dictation.Outcome.Text -> {
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

    private fun copy(text: String) {
        getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("Silent Voice", text.trim()))
    }

    private fun toast(message: String) = Toast.makeText(this, message, Toast.LENGTH_LONG).show()

    /** Logs only HOW the insert went, never the text. */
    private fun done(how: String, notice: String?) {
        Log.i(TAG, "insert: $how")
        if (notice != null) toast(notice)
    }
}

/**
 * The bubble itself, drawn by hand so each state reads at a glance:
 * idle = mic, listening = a halo that swells with your voice around a stop
 * square, working = a turning arc.
 */
class BubbleView(context: Context) : View(context) {
    var phase = Dictation.Phase.Idle
        set(value) {
            field = value
            contentDescription = when (value) {
                Dictation.Phase.Idle -> "Start dictating with Silent Voice"
                Dictation.Phase.Recording -> "Finish dictating"
                Dictation.Phase.Working -> "Silent Voice is writing"
            }
            invalidate()
        }
    var level = 0f

    private val orange = 0xFFF97316.toInt()
    private val fill = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = orange }
    private val halo = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = orange; alpha = 70 }
    private val white = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = 0xFFFFFFFF.toInt() }
    private val arc = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = 0xFFFFFFFF.toInt()
        style = Paint.Style.STROKE
        strokeCap = Paint.Cap.ROUND
    }
    private val mic = context.getDrawable(R.drawable.ic_mic)!!
    private val box = RectF()
    private var shownLevel = 0f

    init {
        contentDescription = "Start dictating with Silent Voice"
    }

    override fun onDraw(c: Canvas) {
        val cx = width / 2f
        val cy = height / 2f
        val outer = minOf(cx, cy)
        val core = outer * 0.76f
        when (phase) {
            Dictation.Phase.Idle -> {
                c.drawCircle(cx, cy, core, fill)
                val h = (core * 0.56f).toInt()
                mic.setBounds((cx - h).toInt(), (cy - h).toInt(), (cx + h).toInt(), (cy + h).toInt())
                mic.draw(c)
            }
            Dictation.Phase.Recording -> {
                // Ease toward the live level so the halo swells instead of flickering.
                shownLevel += (level - shownLevel) * 0.25f
                c.drawCircle(cx, cy, core + (outer - core) * (0.25f + 0.75f * shownLevel), halo)
                c.drawCircle(cx, cy, core, fill)
                val s = core * 0.34f
                box.set(cx - s, cy - s, cx + s, cy + s)
                c.drawRoundRect(box, s * 0.3f, s * 0.3f, white)
                postInvalidateOnAnimation()
            }
            Dictation.Phase.Working -> {
                c.drawCircle(cx, cy, core, fill)
                val r = core * 0.5f
                arc.strokeWidth = core * 0.13f
                box.set(cx - r, cy - r, cx + r, cy + r)
                val angle = (SystemClock.uptimeMillis() % 900L) / 900f * 360f
                c.drawArc(box, angle, 270f, false, arc)
                postInvalidateOnAnimation()
            }
        }
    }
}
