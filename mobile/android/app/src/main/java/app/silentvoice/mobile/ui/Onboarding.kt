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
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
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
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import app.silentvoice.mobile.Dictation
import app.silentvoice.mobile.Keepalive
import app.silentvoice.mobile.ModelDownload
import androidx.lifecycle.repeatOnLifecycle
import kotlinx.coroutines.delay

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
    // Only while the app is on screen: a plain loop kept waking the phone every
    // 600 ms after the user had left the app. Ticks at once on return, so a
    // switch flipped in Settings shows straight away.
    val lifecycle = androidx.lifecycle.compose.LocalLifecycleOwner.current.lifecycle
    val tick by produceState(0, lifecycle) {
        lifecycle.repeatOnLifecycle(androidx.lifecycle.Lifecycle.State.RESUMED) {
            while (true) {
                value++
                delay(600)
            }
        }
    }
    return tick
}

private const val STEPS = 6

/** Six short steps: welcome, microphone, voice model, bubble, keep it running, try it. */
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
                    4 -> KeepStep(tick) { step = 5 }
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

@Composable
private fun Welcome(next: () -> Unit) {
    Page(
        title = "Say it.\nIt's typed.",
        body = "Silent Voice turns what you say into clean text, in any app you type in.",
        action = "Get started",
        onAction = next,
        hero = {
            Logo(40.dp)
            Spacer(Modifier.height(36.dp))
            Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                PillDemo(Modifier.size(120.dp))
            }
            Spacer(Modifier.height(36.dp))
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
    val state = tick.let { Keepalive.bubble(ctx) }
    val on = state == Keepalive.Bubble.On
    Page(
        title = "Meet\nthe bubble.",
        body = "Switch on Silent Voice in Accessibility and this bubble floats above your keyboard, in every app.",
        action = if (on) "Continue" else "Open Accessibility",
        onAction = { if (on) next() else openAccessibility(ctx) },
        secondary = if (!on) {
            { TextButton(onClick = next) { Text("Skip for now") } }
        } else null,
    ) {
        PillView(Dictation.Phase.Idle, Modifier.size(56.dp))
        Spacer(Modifier.height(20.dp))
        if (on) {
            DoneRow("The bubble is on")
        } else if (state == Keepalive.Bubble.Stopped) {
            Hint("1", "Tap Open Accessibility, then Downloaded apps.")
            Hint("2", "Switch Silent Voice bubble off, then on again.")
        } else {
            Hint("1", "Tap Open Accessibility, then Downloaded apps.")
            Hint("2", "Choose Silent Voice bubble and switch it on.")
            Spacer(Modifier.height(12.dp))
            Text(
                "It only types into the field you're in. It never touches password fields, and nothing on your screen is saved or sent.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(16.dp))
            Text(
                "Switch greyed out? Open App info, tap the ⋮ menu, then Allow restricted settings.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            TextButton(onClick = { openAppInfo(ctx) }, modifier = Modifier.offset(x = (-12).dp)) {
                Text("Open App info")
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
private fun KeepStep(tick: Int, next: () -> Unit) {
    val ctx = LocalContext.current
    val safe = tick.let { Keepalive.protected(ctx) }
    Page(
        title = "Keep it\nrunning.",
        body = "Phones close apps they think you're finished with, and that takes the bubble away. " +
            if (Keepalive.isXiaomi) "Two switches stop that." else "One switch stops that.",
        action = "Continue",
        onAction = next,
        secondary = if (!safe) {
            { TextButton(onClick = next) { Text("Skip for now") } }
        } else null,
    ) {
        Surface(color = MaterialTheme.colorScheme.surfaceContainerLow, shape = MaterialTheme.shapes.large) {
            Column(Modifier.padding(vertical = 4.dp)) { KeepAliveRows(tick) }
        }
        if (safe) {
            Spacer(Modifier.height(16.dp))
            DoneRow("Your phone will leave it alone")
        }
    }
}

@Composable
private fun TryStep(done: () -> Unit) {
    val ctx = LocalContext.current
    var text by rememberSaveable { mutableStateOf("") }
    val bubble = Keepalive.bubble(ctx) == Keepalive.Bubble.On
    Page(
        title = "Go on,\nsay something.",
        body = if (bubble) "Tap the box, then tap the bubble above your keyboard. Tap it again when you've finished talking."
        else "Tap the bubble, talk, then tap it again. Turn the bubble on later in Settings to do this in any app.",
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
