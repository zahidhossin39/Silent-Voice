package app.silentvoice.mobile.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameMillis
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.silentvoice.mobile.Dictation
import app.silentvoice.mobile.Pill
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlin.math.sin

/**
 * The bubble (circle with the desktop pill's animation) inside the app. [levels] drives the bars while recording
 * (the microphone by default); bump [doneKey] to flash the green check.
 */
@Composable
fun PillView(
    phase: Dictation.Phase,
    modifier: Modifier = Modifier,
    doneKey: Int = 0,
    levels: Flow<Float> = Dictation.level,
) {
    val density = LocalDensity.current.density
    val pill = remember { Pill(density) }
    val kick = remember { Channel<Unit>(Channel.CONFLATED) }
    var frame by remember { mutableLongStateOf(0L) }
    pill.phase = phase

    LaunchedEffect(phase) { kick.trySend(Unit) }
    LaunchedEffect(doneKey) {
        if (doneKey > 0) {
            pill.done()
            kick.trySend(Unit)
        }
    }
    LaunchedEffect(levels) {
        levels.collect {
            pill.push(it)
            kick.trySend(Unit)
        }
    }
    // Redraw every frame while something moves; sleep once it settles.
    LaunchedEffect(Unit) {
        while (true) {
            withFrameMillis { frame = it }
            if (!pill.animating) kick.receive()
        }
    }
    Canvas(modifier.clearAndSetSemantics { }) {
        val now = frame
        drawIntoCanvas { pill.draw(it.nativeCanvas, size.width, size.height, now) }
    }
}

/**
 * The bubble acting out one dictation on a loop: waiting line, the listening
 * pulse, a few seconds of talking, writing, then the check. Shows a new user
 * what the bubble will do before they meet it.
 */
@Composable
fun PillDemo(modifier: Modifier = Modifier) {
    var phase by remember { androidx.compose.runtime.mutableStateOf(Dictation.Phase.Idle) }
    var done by remember { androidx.compose.runtime.mutableIntStateOf(0) }
    val talk = remember { MutableSharedFlow<Float>(extraBufferCapacity = 8) }
    LaunchedEffect(Unit) {
        while (true) {
            phase = Dictation.Phase.Idle; delay(1400)
            phase = Dictation.Phase.Recording; delay(1100)
            val start = System.currentTimeMillis()
            while (System.currentTimeMillis() - start < 2600) {
                val t = (System.currentTimeMillis() - start) / 1000f
                // Syllable-like bursts: two sines beating, with short gaps.
                val v = (0.55f + 0.45f * sin(t * 11f)) * (0.5f + 0.5f * sin(t * 3.1f + 1f))
                talk.emit(if (sin(t * 1.7f) > -0.85f) v.coerceIn(0f, 1f) else 0f)
                delay(50)
            }
            phase = Dictation.Phase.Working; delay(1300)
            phase = Dictation.Phase.Idle; done++; delay(900)
        }
    }
    PillView(phase, modifier, doneKey = done, levels = talk)
}

/** The app icon from the desktop (app-icon.svg): five bars, a white peak, on near-black. */
@Composable
fun Logo(size: Dp = 32.dp, modifier: Modifier = Modifier) {
    Canvas(modifier.size(size).clearAndSetSemantics { }) {
        val u = this.size.minDimension / 1024f
        drawRoundRect(Color(0xFF0D0F14), cornerRadius = CornerRadius(224 * u))
        val bars = listOf(232f to 160f, 360f to 320f, 488f to 520f, 616f to 320f, 744f to 160f)
        bars.forEachIndexed { i, (x, h) ->
            drawRoundRect(
                if (i == 2) Color.White else Brand,
                topLeft = Offset(x * u, (512f - h / 2) * u),
                size = Size(80 * u, h * u),
                cornerRadius = CornerRadius(40 * u),
            )
        }
    }
}

/** Logo and name, the header every main screen starts with. */
@Composable
fun Wordmark(modifier: Modifier = Modifier) {
    Row(modifier, verticalAlignment = Alignment.CenterVertically) {
        Logo(24.dp)
        Spacer(Modifier.width(10.dp))
        Text("Silent Voice", style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurface)
    }
}
