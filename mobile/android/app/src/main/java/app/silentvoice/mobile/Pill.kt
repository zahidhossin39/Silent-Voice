package app.silentvoice.mobile

import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import kotlin.math.abs
import kotlin.math.exp
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.roundToInt

/**
 * The phone bubble: a dark circle that shows a mic while waiting and the
 * desktop pill's bars while listening (src/components/overlay/RecordingOverlay.tsx),
 * with an orange ring outside that swells with the voice. Picked by the user
 * from `mobile/android/prototype/bubble-prototype.html` (idle A + talking B,
 * kept on the branch `prototype/bubble`). Used by the floating bubble and the
 * in-app mic.
 *
 *   idle      a white mic, still (a bubble that always moves distracts and costs battery)
 *   recording orange bars; a staggered pulse while quiet, a live envelope while talking.
 *             Outside the circle: a ring that swells with the voice, and a ripple
 *             every 1.5 s while quiet ("still listening")
 *   working   grey bars pulsing slower
 *   done      bars fade and a green check shows for 700 ms
 *
 * Sizes are the desktop's pixels, scaled so 30 of them span the circle. The ring
 * and its ripple reach 3.6 of those units outside the circle, so callers leave that much room
 * ([draw]'s inset). One instance per view; call [draw] every frame while
 * [animating] is true.
 */
class Pill(private val density: Float) {
    var phase = Dictation.Phase.Idle
        set(value) {
            if (field == value) return
            field = value
            if (value != Dictation.Phase.Recording) hist.fill(0f)
        }

    /** Desktop sizes: the waiting heights, and its colours. */
    private val base = floatArrayOf(6f, 10f, 13f, 9f, 5f)
    private val accent = 0xFFF97316.toInt()
    private val muted = 0xFF8B93A7.toInt()
    private val good = 0xFF22C55E.toInt()
    private val micColor = 0xFFE6E9F0.toInt()
    val fill = 0xFF0E1116.toInt()
    // A step lighter than the desktop's #262C3D: a circle on a dark phone
    // screen needs a clearer edge than a pill on a desktop.
    private val border = 0xFF343B50.toInt()
    private val gap = 2f

    /** Recent loudness, oldest first; each new reading shifts the bars left. */
    private val hist = FloatArray(5)
    private val w = FloatArray(5) { 2f }
    private val h = FloatArray(5) { 2f }
    private val alpha = FloatArray(5)
    /** 0 = grey, 1 = orange; bars only ever move between the two. */
    private val warm = FloatArray(5)
    private var check = 0f
    /** How much of the mic shows (1 = waiting); the bars fade in as it fades out. */
    private var idleShown = 1f
    /** How much of the outer ring shows (1 = listening). */
    private var ringShown = 0f
    /** Loudness for the ring: rises fast, falls slowly. */
    private var envelope = 0f
    /** 1 while talking, 0 while quiet; the ripple shows only while quiet. */
    private var talking = 0f
    private var wasRecording = false
    private var rippleStart = 0L
    private var doneUntil = 0L
    private var donePending = false
    private var pulseKind = 0 // 0 none, 1 fast (recording), 2 slow (working)
    private var pulseStart = 0L
    private var last = 0L

    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val stroke = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeCap = Paint.Cap.ROUND
        strokeJoin = Paint.Join.ROUND
    }
    private val rect = RectF()
    private val tick = Path()

    /** True while anything is still moving, so the view keeps redrawing. */
    var animating = true
        private set

    /** A new loudness reading, 0..1 (see [Dictation.level]). */
    fun push(level: Float) {
        if (phase != Dictation.Phase.Recording) return
        System.arraycopy(hist, 1, hist, 0, 4)
        hist[4] = level
        animating = true
    }

    /** The text landed: show the green check for 700 ms from the next frame. */
    fun done() {
        donePending = true
        animating = true
    }

    /**
     * Draws the bubble filling [width] x [height] pixels, less [inset] on every
     * side (room for the ring and the shadow). [shadow] gives it a soft lift
     * for the floating bubble.
     */
    fun draw(c: Canvas, width: Float, height: Float, now: Long, inset: Float = 0f, shadow: Boolean = false) {
        val dt = if (last == 0L) 16L else (now - last).coerceIn(0L, 100L)
        last = now
        if (donePending) {
            donePending = false
            doneUntil = now + 700
        }
        val done = now < doneUntil
        val recording = phase == Dictation.Phase.Recording && !done
        val working = phase == Dictation.Phase.Working && !done
        val idle = !recording && !working && !done
        val speaking = recording && hist.max() > 0.14f
        if (recording && !wasRecording) rippleStart = now
        wasRecording = recording

        // Pulse restarts whenever it switches on, like re-adding the CSS class.
        val kind = when {
            recording && !speaking -> 1
            working -> 2
            else -> 0
        }
        if (kind != pulseKind) {
            pulseKind = kind
            pulseStart = now
        }

        // CSS: transition-all 300 ms ease-out; 80 ms while talking.
        val k = ease(dt, if (speaking) 20f else 75f)
        var moving = kind != 0 || done || recording
        for (i in 0 until 5) {
            val th: Float
            val ta: Float
            val tc: Float
            when {
                done -> { th = 2f; ta = 0f; tc = warm[i] }
                idle -> { th = 2f; ta = 0f; tc = 0f }
                recording -> {
                    th = if (speaking) max(2f, (2f + hist[i] * 13f).roundToInt().toFloat()) else base[i]
                    ta = 1f
                    tc = 1f
                }
                else -> { th = base[i]; ta = 1f; tc = 0f }
            }
            h[i] += (th - h[i]) * k
            alpha[i] += (ta - alpha[i]) * k
            warm[i] += (tc - warm[i]) * k
            if (abs(th - h[i]) > 0.02f || abs(ta - alpha[i]) > 0.01f || abs(tc - warm[i]) > 0.01f) moving = true
        }
        check += ((if (done) 1f else 0f) - check) * ease(dt, 40f)
        // The mic comes back only once the check has nearly gone, so they never overlap.
        val ti = if (idle && check < 0.15f) 1f else 0f
        idleShown += (ti - idleShown) * ease(dt, 90f)
        val tr = if (recording) 1f else 0f
        ringShown += (tr - ringShown) * ease(dt, 90f)
        talking += ((if (speaking) 1f else 0f) - talking) * ease(dt, if (speaking) 60f else 160f)
        val te = if (recording) hist[4] else 0f
        envelope += (te - envelope) * ease(dt, if (te > envelope) 30f else 160f)
        if (check in 0.01f..0.99f || abs(ti - idleShown) > 0.01f || abs(tr - ringShown) > 0.01f ||
            talking in 0.01f..0.99f || envelope > 0.005f
        ) moving = true
        animating = moving

        val d = min(width, height) - inset * 2
        val cx = width / 2
        val cy = height / 2
        val s = d / 30f
        paint.color = fill
        if (shadow) paint.setShadowLayer(4f * density, 0f, 1.5f * density, 0x55000000) else paint.clearShadowLayer()
        c.drawCircle(cx, cy, d / 2, paint)
        paint.clearShadowLayer()
        val bw = max(1f, density * 0.75f)
        stroke.color = border
        stroke.strokeWidth = bw
        c.drawCircle(cx, cy, d / 2 - bw / 2, stroke)

        // Outer ring: swells with the voice; a ripple leaves it while quiet.
        if (ringShown > 0.01f) {
            val e = min(1f, envelope * 1.3f)
            stroke.color = accent
            stroke.alpha = (ringShown * (0.25f + 0.75f * e) * 255).roundToInt().coerceIn(0, 255)
            stroke.strokeWidth = (1f + 0.6f * e) * s
            c.drawCircle(cx, cy, (15.8f + 1.6f * e) * s, stroke)
            val p = ((now - rippleStart) % 1500L) / 1500f
            val ra = ringShown * (1f - talking) * (1f - p) * 0.7f
            if (ra > 0.01f) {
                stroke.alpha = (ra * 255).roundToInt().coerceIn(0, 255)
                stroke.strokeWidth = 0.9f * s
                c.drawCircle(cx, cy, (15.5f + p * 2.6f) * s, stroke)
            }
        }

        // The mic, shrinking slightly as it fades.
        if (idleShown > 0.01f) drawMic(c, cx, cy, s * 0.95f * (0.82f + 0.18f * idleShown), idleShown)

        // Bars, centred as a flex row, growing in as the mic goes.
        val shown = 1f - idleShown
        if (shown > 0.01f) {
            c.save()
            val sc = 0.85f + 0.15f * shown
            c.scale(sc, sc, cx, cy)
            var x = cx - (w.sum() + gap * 4) * s / 2
            for (i in 0 until 5) {
                val bwi = w[i] * s
                var bh = h[i] * s
                var a = alpha[i]
                if (pulseKind != 0) {
                    val (scale, op) = pulse(now - pulseStart, i, if (pulseKind == 1) 1000f else 1600f)
                    bh *= scale
                    a *= op
                }
                a *= shown
                if (a > 0.01f) {
                    paint.color = mix(muted, accent, warm[i])
                    paint.alpha = (a * 255).roundToInt().coerceIn(0, 255)
                    rect.set(x, cy - bh / 2, x + bwi, cy + bh / 2)
                    val r = min(bwi, bh) / 2
                    c.drawRoundRect(rect, r, r, paint)
                }
                x += bwi + gap * s
            }
            c.restore()
        }

        // Green check, 14 px in a 24-unit box, stroke 3.2 (desktop SVG).
        if (check > 0.01f) {
            val size = 14f * s
            val u = size / 24f
            val ox = cx - size / 2
            val oy = cy - size / 2
            tick.reset()
            tick.moveTo(ox + 20 * u, oy + 6 * u)
            tick.lineTo(ox + 9 * u, oy + 17 * u)
            tick.lineTo(ox + 4 * u, oy + 12 * u)
            stroke.color = good
            stroke.alpha = (check * 255).roundToInt()
            stroke.strokeWidth = 3.2f * u
            c.drawPath(tick, stroke)
        }
    }

    /** A classic mic: capsule, holder arc, stem and foot, in units of [scale] pixels. */
    private fun drawMic(c: Canvas, cx: Float, cy: Float, scale: Float, shown: Float) {
        val a = (shown * 255).roundToInt().coerceIn(0, 255)
        c.save()
        c.translate(cx, cy)
        c.scale(scale, scale)
        c.translate(0f, 0.75f)
        paint.color = micColor
        paint.alpha = a
        rect.set(-3.2f, -9f, 3.2f, 2f)
        c.drawRoundRect(rect, 3.2f, 3.2f, paint)
        stroke.color = micColor
        stroke.alpha = a
        stroke.strokeWidth = 1.8f
        rect.set(-6f, -8.6f, 6f, 3.4f)
        c.drawArc(rect, 0f, 180f, false, stroke)
        c.drawLine(0f, 3.4f, 0f, 6.6f, stroke)
        c.drawLine(-3f, 6.6f, 3f, 6.6f, stroke)
        c.restore()
    }

    private fun ease(dt: Long, tau: Float) = 1f - exp(-dt / tau)

    /**
     * The sv-pulse keyframes: scaleY 0.4 -> 1 -> 0.4 and opacity 0.6 -> 1 -> 0.6,
     * ease-in-out, each bar starting 0.1 s after the one before.
     */
    private fun pulse(elapsed: Long, i: Int, period: Float): Pair<Float, Float> {
        val t = elapsed - i * 100f
        if (t < 0) return 1f to 1f // CSS holds the bar unanimated during its delay
        val p = (t % period) / period
        val u = if (p < 0.5f) p / 0.5f else (1f - p) / 0.5f
        val e = u * u * (3f - 2f * u)
        return (0.4f + 0.6f * e) to (0.6f + 0.4f * e)
    }

    private fun mix(a: Int, b: Int, t: Float): Int {
        fun ch(shift: Int): Int {
            val x = (a shr shift) and 0xFF
            val y = (b shr shift) and 0xFF
            return (x + (y - x) * t.coerceIn(0f, 1f)).roundToInt()
        }
        return (ch(24) shl 24) or (ch(16) shl 16) or (ch(8) shl 8) or ch(0)
    }

    companion object {
        /** How far the ring reaches outside the circle, as a share of its diameter. */
        const val RING_ROOM = 3.6f / 30f

        /**
         * Desktop loudness mapping (RecordingOverlay normLevel): the backend's
         * rms*300 level, floor 3, span 26, curve 0.7.
         */
        fun norm(rms: Float): Float = ((rms * 300f - 3f) / 26f).coerceIn(0f, 1f).pow(0.7f)
    }
}
