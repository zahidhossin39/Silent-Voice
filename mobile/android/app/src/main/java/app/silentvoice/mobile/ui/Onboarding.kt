package app.silentvoice.mobile.ui

import android.Manifest
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import app.silentvoice.mobile.Dictation
import app.silentvoice.mobile.ModelDownload
import app.silentvoice.mobile.R
import kotlinx.coroutines.delay

fun bubbleEnabled(ctx: Context): Boolean =
    Settings.Secure.getString(ctx.contentResolver, Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES)
        .orEmpty().contains(ctx.packageName)

fun openAccessibility(ctx: Context) =
    ctx.startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))

fun openAppInfo(ctx: Context) = ctx.startActivity(
    Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:${ctx.packageName}"))
        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
)

/**
 * A counter that ticks while the screen is shown. Permissions and the
 * accessibility switch change outside the app and send no event, so screens
 * that show their state read this to re-check.
 */
@Composable
fun rememberTick(): Int {
    val tick by produceState(0) {
        while (true) {
            delay(600)
            value++
        }
    }
    return tick
}

private const val STEPS = 5

/** Five short steps: welcome, microphone, voice model, bubble, try it. */
@Composable
fun Onboarding(onDone: () -> Unit) {
    val ctx = LocalContext.current
    var step by rememberSaveable { mutableIntStateOf(0) }
    val tick = rememberTick()
    BackHandler(enabled = step > 0) { step-- }

    Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
        Column(Modifier.fillMaxSize().safeDrawingPadding().padding(horizontal = 24.dp)) {
            StepBar(step)
            AnimatedContent(
                targetState = step,
                modifier = Modifier.weight(1f),
                transitionSpec = {
                    val dir = if (targetState > initialState) 1 else -1
                    (slideInHorizontally(tween(260)) { it / 6 * dir } + fadeIn(tween(260))) togetherWith
                        (slideOutHorizontally(tween(160)) { -it / 6 * dir } + fadeOut(tween(160)))
                },
                label = "onboarding",
            ) { page ->
                when (page) {
                    0 -> Welcome { step = 1 }
                    1 -> MicStep(tick) { step = 2 }
                    2 -> ModelStep(tick) { step = 3 }
                    3 -> BubbleStep(tick) { step = 4 }
                    else -> TryStep(onDone)
                }
            }
        }
    }
}

@Composable
private fun StepBar(step: Int) {
    Row(
        Modifier.fillMaxWidth().padding(top = 16.dp, bottom = 8.dp)
            .clearAndSetSemantics { },
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        repeat(STEPS) { i ->
            val on by animateFloatAsState(if (i <= step) 1f else 0f, tween(300), label = "step")
            Box(
                Modifier.weight(1f).height(4.dp).clip(CircleShape)
                    .background(MaterialTheme.colorScheme.surfaceContainerHighest)
            ) {
                Box(Modifier.fillMaxWidth(on).height(4.dp).background(MaterialTheme.colorScheme.primary))
            }
        }
    }
}

/** Shared page frame: scrolling content on top, one clear action pinned at the bottom. */
@Composable
private fun Page(
    title: String,
    body: String,
    action: String,
    onAction: () -> Unit,
    actionEnabled: Boolean = true,
    hero: @Composable () -> Unit = {},
    secondary: (@Composable () -> Unit)? = null,
    content: @Composable () -> Unit = {},
) {
    Column(Modifier.fillMaxSize()) {
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState())) {
            Spacer(Modifier.height(24.dp))
            hero()
            Text(title, style = MaterialTheme.typography.displaySmall, color = MaterialTheme.colorScheme.onSurface)
            Spacer(Modifier.height(12.dp))
            Text(body, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(24.dp))
            content()
            Spacer(Modifier.height(16.dp))
        }
        Button(
            onClick = onAction,
            enabled = actionEnabled,
            modifier = Modifier.fillMaxWidth().heightIn(min = 56.dp),
        ) { Text(action, style = MaterialTheme.typography.titleMedium) }
        Box(Modifier.fillMaxWidth().heightIn(min = 56.dp), contentAlignment = Alignment.Center) {
            secondary?.invoke()
        }
    }
}

/** The brand mark with slow breathing rings: the bubble, before the user has met it. */
@Composable
fun MicHero(size: Int = 132, breathing: Boolean = true) {
    val pulse = if (breathing) {
        rememberInfiniteTransition(label = "hero")
            .animateFloat(0f, 1f, infiniteRepeatable(tween(2400), RepeatMode.Restart), label = "pulse").value
    } else 0f
    Box(Modifier.size(size.dp).clearAndSetSemantics { }, contentAlignment = Alignment.Center) {
        Canvas(Modifier.fillMaxSize()) {
            val core = this.size.minDimension * 0.30f
            val max = this.size.minDimension * 0.5f
            for (k in 0..1) {
                val t = (pulse + k * 0.5f) % 1f
                drawCircle(Brand.copy(alpha = 0.22f * (1f - t)), radius = core + (max - core) * t)
            }
            drawCircle(Brand, radius = core)
        }
        Icon(
            painterResource(R.drawable.ic_mic), null,
            tint = androidx.compose.ui.graphics.Color.White,
            modifier = Modifier.size((size * 0.3f).dp),
        )
    }
}

@Composable
private fun Welcome(next: () -> Unit) {
    Page(
        title = "Say it.\nIt's typed.",
        body = "Silent Voice turns what you say into clean text, in any app you type in.",
        action = "Get started",
        onAction = next,
        hero = {
            MicHero()
            Spacer(Modifier.height(20.dp))
        },
    ) {
        Promise("Runs on your phone", "Your voice becomes text right here. No server listening in.")
        Promise("Works offline", "On a plane, in a tunnel, out of data. Still works.")
        Promise("Free, no account", "Nothing to sign up for. Nothing to cancel.")
    }
}

@Composable
private fun Promise(title: String, body: String) {
    Row(Modifier.padding(vertical = 8.dp), verticalAlignment = Alignment.Top) {
        Box(
            Modifier.size(28.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primaryContainer),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                Icons.Rounded.Check, null,
                tint = MaterialTheme.colorScheme.onPrimaryContainer,
                modifier = Modifier.size(18.dp),
            )
        }
        Column(Modifier.padding(start = 14.dp)) {
            Text(title, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurface)
            Text(body, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@Composable
private fun DoneRow(text: String) {
    Row(
        Modifier.fillMaxWidth().clip(MaterialTheme.shapes.medium)
            .background(MaterialTheme.colorScheme.tertiaryContainer).padding(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(Icons.Rounded.Check, null, tint = MaterialTheme.colorScheme.onTertiaryContainer)
        Text(
            text,
            Modifier.padding(start = 12.dp),
            style = MaterialTheme.typography.titleSmall,
            color = MaterialTheme.colorScheme.onTertiaryContainer,
        )
    }
}

@Composable
private fun MicStep(tick: Int, next: () -> Unit) {
    val ctx = LocalContext.current
    var refused by rememberSaveable { mutableStateOf(false) }
    val granted = tick.let { Dictation.micGranted(ctx) }
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok -> refused = !ok }
    Page(
        title = "First,\nyour voice.",
        body = "Silent Voice listens only while you're dictating. The sound is turned into text on this phone, then thrown away.",
        action = if (granted) "Continue" else "Allow microphone",
        onAction = { if (granted) next() else ask.launch(Manifest.permission.RECORD_AUDIO) },
        secondary = if (refused && !granted) {
            { TextButton(onClick = { openAppInfo(ctx) }) { Text("Open app settings") } }
        } else null,
    ) {
        if (granted) DoneRow("Microphone is on")
        else if (refused) Text(
            "Android won't ask again after a refusal. Open app settings, tap Permissions, and allow the microphone.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.error,
        )
    }
}

@Composable
private fun ModelStep(tick: Int, next: () -> Unit) {
    val ctx = LocalContext.current
    val progress by ModelDownload.progress.collectAsState()
    val error by ModelDownload.error.collectAsState()
    val ready = tick.let { Dictation.modelReady(ctx) } && progress == null
    Page(
        title = "A brain that fits\nin your pocket.",
        body = "One download, about 100 MB. After that, Silent Voice understands you without the internet.",
        action = when {
            ready -> "Continue"
            progress != null -> "Downloading…"
            error != null -> "Try again"
            else -> "Download (100 MB)"
        },
        actionEnabled = progress == null,
        onAction = { if (ready) next() else ModelDownload.start(ctx) },
    ) {
        val p = progress
        when {
            ready -> DoneRow("Voice model is on your phone")
            p != null -> Column {
                LinearProgressIndicator(progress = { p }, modifier = Modifier.fillMaxWidth().height(8.dp).clip(CircleShape))
                Spacer(Modifier.height(10.dp))
                Text(
                    "${(p * 100).toInt()}%  ·  keep this screen open",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            error != null -> Text(error!!, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error)
        }
    }
}

@Composable
private fun BubbleStep(tick: Int, next: () -> Unit) {
    val ctx = LocalContext.current
    val on = tick.let { bubbleEnabled(ctx) }
    Page(
        title = "Meet\nthe bubble.",
        body = "Switch on Silent Voice in Accessibility and an orange mic appears above your keyboard, in every app.",
        action = if (on) "Continue" else "Open Accessibility",
        onAction = { if (on) next() else openAccessibility(ctx) },
        secondary = if (!on) {
            { TextButton(onClick = next) { Text("Skip for now") } }
        } else null,
    ) {
        if (on) {
            DoneRow("The bubble is on")
        } else {
            Hint("1", "Tap Open Accessibility, then Downloaded apps.")
            Hint("2", "Choose Silent Voice bubble and switch it on.")
            Spacer(Modifier.height(12.dp))
            Text(
                "It only types into the field you're in. It never touches password fields, and nothing on your screen is saved or sent.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(8.dp))
            TextButton(onClick = { openAppInfo(ctx) }, contentPadding = androidx.compose.foundation.layout.PaddingValues(0.dp)) {
                Text("Switch greyed out? Open App info, tap ⋮, Allow restricted settings")
            }
        }
    }
}

@Composable
private fun Hint(n: String, text: String) {
    Row(Modifier.padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(
            Modifier.size(28.dp).clip(CircleShape).background(MaterialTheme.colorScheme.secondaryContainer),
            contentAlignment = Alignment.Center,
        ) {
            Text(n, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSecondaryContainer)
        }
        Text(
            text,
            Modifier.padding(start = 14.dp),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurface,
        )
    }
}

@Composable
private fun TryStep(done: () -> Unit) {
    val ctx = LocalContext.current
    var text by rememberSaveable { mutableStateOf("") }
    val bubble = bubbleEnabled(ctx)
    Page(
        title = "Go on,\nsay something.",
        body = if (bubble) "Tap the box, then tap the orange bubble. Tap it again when you've finished talking."
        else "Tap the mic, talk, then tap it again. Turn the bubble on later in Settings to do this in any app.",
        action = "Start using Silent Voice",
        onAction = done,
    ) {
        OutlinedTextField(
            value = text,
            onValueChange = { text = it },
            placeholder = { Text("Your words land here") },
            minLines = 4,
            modifier = Modifier.fillMaxWidth(),
            shape = MaterialTheme.shapes.large,
        )
        if (!bubble) {
            Spacer(Modifier.height(16.dp))
            Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                InAppMic { text = (text.trimEnd() + " " + it).trim() }
            }
        }
        Spacer(Modifier.height(12.dp))
        Text(
            "Tip: say \"new line\" to start a new line.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Start,
        )
    }
}
