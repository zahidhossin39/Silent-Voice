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
 * The phone bubble: a dark circle with the desktop pill's animation inside
 * (src/components/overlay/RecordingOverlay.tsx), so it moves exactly like the
 * Windows pill. Used by the floating bubble and the in-app mic.
 *
 * The same five bars are always drawn and morph between states:
 *   idle      one thin grey line in the middle
 *   recording orange bars; a staggered pulse while quiet, a live envelope while talking
 *   working   grey bars pulsing slower
 *   done      bars collapse and a green check shows for 700 ms
 *
 * All sizes are the desktop's pixels, scaled so 30 of them span the circle. One
 * instance per view; call [draw] every frame while [animating] is true.
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
    val fill = 0xFF0E1116.toInt()
    // A step lighter than the desktop's #262C3D: a circle on a dark phone
    // screen needs a clearer edge than a pill on a desktop.
    private val border = 0xFF343B50.toInt()

    /** Recent loudness, oldest first; each new reading shifts the bars left. */
    private val hist = FloatArray(5)
    private val w = FloatArray(5)
    private val h = FloatArray(5) { 2f }
    private val alpha = FloatArray(5)
    /** 0 = grey, 1 = orange; bars only ever move between the two. */
    private val warm = FloatArray(5)
    private var gap = 0f
    private var check = 0f
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
     * Draws the pill filling [width] x [height] pixels. [shadow] gives it a soft
     * lift for the floating bubble.
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
        val k = 1f - exp(-dt / (if (speaking) 20f else 75f))
        var moving = kind != 0 || done
        for (i in 0 until 5) {
            val tw: Float
            val th: Float
            val ta: Float
            val tc: Float
            when {
                done -> { tw = 0f; th = 2f; ta = 0f; tc = warm[i] }
                idle -> { tw = if (i == 2) 20f else 0f; th = 2f; ta = if (i == 2) 1f else 0f; tc = 0f }
                recording -> {
                    tw = 2f
                    th = if (speaking) max(2f, (2f + hist[i] * 13f).roundToInt().toFloat()) else base[i]
                    ta = 1f
                    tc = 1f
                }
                else -> { tw = 2f; th = base[i]; ta = 1f; tc = 0f }
            }
            w[i] += (tw - w[i]) * k
            h[i] += (th - h[i]) * k
            alpha[i] += (ta - alpha[i]) * k
            warm[i] += (tc - warm[i]) * k
            if (abs(tw - w[i]) > 0.02f || abs(th - h[i]) > 0.02f ||
                abs(ta - alpha[i]) > 0.01f || abs(tc - warm[i]) > 0.01f
            ) moving = true
        }
        val tg = if (idle || done) 0f else 2f
        gap += (tg - gap) * k
        check += ((if (done) 1f else 0f) - check) * (1f - exp(-dt / 40f))
        if (abs(tg - gap) > 0.02f || check in 0.01f..0.99f) moving = true
        animating = moving

        // Round bubble (the phone keeps its circle; only the inside is the pill).
        // The desktop's 22 px pill content fits in a 30-unit circle.
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

        // Bars, centred as a flex row with `gap` between all five.
        var x = cx - (w.sum() + gap * 4) * s / 2
        for (i in 0 until 5) {
            val bwi = w[i] * s
            var bh = h[i] * s
            var a = alpha[i]
            if (pulseKind != 0) {
                val (scale, op) = pulse(now - pulseStart, i, if (pulseKind == 1) 1000f else 1600f)
                bh *= scale
                a = op
            }
            if (bwi > 0.05f && a > 0.01f) {
                paint.color = mix(muted, accent, warm[i])
                paint.alpha = (a * 255).roundToInt().coerceIn(0, 255)
                rect.set(x, cy - bh / 2, x + bwi, cy + bh / 2)
                val r = min(bwi, bh) / 2
                c.drawRoundRect(rect, r, r, paint)
            }
            x += bwi + gap * s
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
        /**
         * Desktop loudness mapping (RecordingOverlay normLevel): the backend's
         * rms*300 level, floor 3, span 26, curve 0.7.
         */
        fun norm(rms: Float): Float = ((rms * 300f - 3f) / 26f).coerceIn(0f, 1f).pow(0.7f)
    }
}
