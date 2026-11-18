package app.silentvoice.mobile.ui

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.Home
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import app.silentvoice.mobile.Ai
import app.silentvoice.mobile.Dictation
import app.silentvoice.mobile.Entry
import app.silentvoice.mobile.History
import app.silentvoice.mobile.Keepalive
import app.silentvoice.mobile.ModelDownload
import app.silentvoice.mobile.Prefs
import app.silentvoice.mobile.R
import java.text.SimpleDateFormat
import java.util.Calendar
import java.util.Date
import java.util.Locale

private enum class Tab(val label: String) { Home("Home"), History("History"), Settings("Settings") }

@Composable
fun MainScreen() {
    var tab by rememberSaveable { mutableIntStateOf(0) }
    var modeSheet by rememberSaveable { mutableStateOf(false) }
    // Bumped whenever a setting changes, so every screen re-reads Prefs.
    var settingsVersion by remember { mutableIntStateOf(0) }
    BackHandler(enabled = tab != 0) { tab = 0 }

    Scaffold(
        containerColor = MaterialTheme.colorScheme.surface,
        bottomBar = {
            NavigationBar(containerColor = MaterialTheme.colorScheme.surfaceContainer) {
                Tab.entries.forEachIndexed { i, t ->
                    val icon: Painter = when (t) {
                        Tab.Home -> rememberVectorPainter(Icons.Rounded.Home)
                        Tab.History -> painterResource(R.drawable.ic_history)
                        Tab.Settings -> rememberVectorPainter(Icons.Rounded.Settings)
                    }
                    NavigationBarItem(
                        selected = tab == i,
                        onClick = { tab = i },
                        icon = { Icon(icon, null) },
                        label = { Text(t.label) },
                    )
                }
            }
        },
    ) { inner ->
        AnimatedContent(
            targetState = tab,
            modifier = Modifier.padding(inner),
            transitionSpec = { fadeIn(tween(200)) togetherWith fadeOut(tween(120)) },
            label = "tab",
        ) { current ->
            when (Tab.entries[current]) {
                Tab.Home -> HomeScreen(settingsVersion, onModes = { modeSheet = true }, onSetup = { tab = 2 })
                Tab.History -> HistoryScreen()
                Tab.Settings -> SettingsScreen(settingsVersion, onChanged = { settingsVersion++ }, onModes = { modeSheet = true })
            }
        }
    }
    if (modeSheet) ModeSheet(onClose = { modeSheet = false; settingsVersion++ })
}

// ---------------------------------------------------------------- shared bits

@Composable
private fun ScreenTitle(eyebrow: String?, title: String) {
    Column(Modifier.padding(top = if (eyebrow == null) 20.dp else 28.dp, bottom = 20.dp)) {
        if (eyebrow != null) {
            Text(eyebrow.uppercase(), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.primary)
            Spacer(Modifier.height(6.dp))
        }
        Text(
            title,
            style = MaterialTheme.typography.displaySmall,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.semantics { heading() },
        )
    }
}

@Composable
private fun SectionLabel(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.titleSmall,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(top = 28.dp, bottom = 8.dp, start = 4.dp).semantics { heading() },
    )
}

/** A rounded surface that groups related rows, the M3 way of separating content without lines. */
@Composable
private fun Group(content: @Composable () -> Unit) {
    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        shape = MaterialTheme.shapes.large,
        modifier = Modifier.fillMaxWidth(),
    ) { Column(Modifier.padding(vertical = 4.dp)) { content() } }
}

@Composable
private fun Row2(
    title: String,
    subtitle: String? = null,
    onClick: (() -> Unit)? = null,
    trailing: @Composable () -> Unit = {},
) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 64.dp)
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(horizontal = 20.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f).padding(end = 12.dp)) {
            Text(title, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurface)
            if (subtitle != null) {
                Text(subtitle, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        trailing()
    }
}

@Composable
private fun SwitchRow(title: String, subtitle: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 64.dp)
            .selectable(selected = checked, role = Role.Switch, onClick = { onChange(!checked) })
            .padding(horizontal = 20.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f).padding(end = 12.dp)) {
            Text(title, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurface)
            Text(subtitle, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Switch(checked = checked, onCheckedChange = null)
    }
}

private fun copyText(ctx: Context, text: String) {
    ctx.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("Silent Voice", text))
    Toast.makeText(ctx, "Copied", Toast.LENGTH_SHORT).show()
}

/**
 * The bubble as a big button that dictates inside the app, without the
 * floating bubble. Lets someone try Silent Voice before that is on.
 */
@Composable
fun InAppMic(onText: (String) -> Unit) {
    val ctx = LocalContext.current
    val phase by Dictation.phase.collectAsState()
    var doneKey by remember { mutableIntStateOf(0) }
    val go = {
        Dictation.toggle(ctx) { outcome ->
            when (outcome) {
                is Dictation.Outcome.Text -> {
                    doneKey++
                    onText(outcome.text)
                    outcome.notice?.let { Toast.makeText(ctx, it, Toast.LENGTH_LONG).show() }
                }
                is Dictation.Outcome.Problem -> Toast.makeText(ctx, outcome.message, Toast.LENGTH_LONG).show()
            }
        }
    }
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok -> if (ok) go() }
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        // An 80 dp bubble with 10 dp round it for the listening ring (80 x Pill.RING_ROOM = 9.6 dp);
        // only the circle itself takes taps.
        Box(Modifier.size(100.dp), contentAlignment = Alignment.Center) {
            PillView(phase, Modifier.fillMaxSize(), doneKey, inset = 10.dp)
            Box(
                Modifier.size(80.dp).clip(CircleShape)
                    .clickable(
                        enabled = phase != Dictation.Phase.Working,
                        role = Role.Button,
                        onClick = {
                            if (Dictation.micGranted(ctx)) go() else ask.launch(Manifest.permission.RECORD_AUDIO)
                        },
                    )
                    .semantics {
                        contentDescription = if (phase == Dictation.Phase.Recording) "Finish dictating" else "Start dictating"
                    },
            )
        }
        Spacer(Modifier.height(2.dp))
        Text(
            when (phase) {
                Dictation.Phase.Idle -> "Tap the bubble to talk"
                Dictation.Phase.Starting -> "Starting the microphone…"
                Dictation.Phase.Recording -> "Listening. Tap again when you're done"
                Dictation.Phase.Working -> "Writing it down…"
            },
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

// ----------------------------------------------------------------------- Home

@Composable
private fun HomeScreen(version: Int, onModes: () -> Unit, onSetup: () -> Unit) {
    val ctx = LocalContext.current
    val tick = rememberTick()
    val items by History.items.collectAsState()
    val bubble = tick.let { Keepalive.bubble(ctx) }
    val safe = tick.let { Keepalive.protected(ctx) }
    val model = tick.let { Dictation.modelReady(ctx) }
    val mic = tick.let { Dictation.micGranted(ctx) }
    var scratch by rememberSaveable { mutableStateOf("") }
    val aiOn = version.let { Prefs.aiOn && Ai.configured() }

    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp)) {
        Wordmark(Modifier.padding(top = 20.dp))
        ScreenTitle(null, if (items.isEmpty()) "Ready when\nyou are." else greeting())

        // One card, one message: either you're set, or here is the single next step.
        val ready = bubble == Keepalive.Bubble.On && model && mic
        Surface(
            color = if (ready) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.errorContainer,
            shape = MaterialTheme.shapes.extraLarge,
            modifier = Modifier.fillMaxWidth(),
        ) {
            val on = if (ready) MaterialTheme.colorScheme.onPrimaryContainer else MaterialTheme.colorScheme.onErrorContainer
            Column(Modifier.padding(24.dp)) {
                if (ready) {
                    Text("The bubble is on", style = MaterialTheme.typography.titleLarge, color = on)
                    Text("Works in every app, even offline.", style = MaterialTheme.typography.bodyMedium, color = on)
                    Spacer(Modifier.height(18.dp))
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        PillView(Dictation.Phase.Idle, Modifier.size(40.dp))
                        Text(
                            "Look for this bubble above your keyboard. Tap it, talk, tap again.",
                            style = MaterialTheme.typography.bodyMedium, color = on,
                            modifier = Modifier.padding(start = 14.dp),
                        )
                    }
                } else {
                    val (title, body, label) = when {
                        !mic -> Triple("Microphone is off", "Silent Voice can't hear you until the microphone is allowed.", "Fix in Settings")
                        !model -> Triple("Voice model missing", "Download the 110 MB voice model to start dictating.", "Go to Settings")
                        bubble == Keepalive.Bubble.Stopped -> Triple(
                            "Your phone stopped the bubble",
                            "It closed Silent Voice in the background. In Accessibility, switch Silent Voice off, then on again.",
                            "Open Accessibility",
                        )
                        else -> Triple("One step left", "Turn on the bubble to dictate in any app. Until then, the bubble below works inside this app.", "Turn on the bubble")
                    }
                    Text(title, style = MaterialTheme.typography.titleLarge, color = on)
                    Spacer(Modifier.height(6.dp))
                    Text(body, style = MaterialTheme.typography.bodyLarge, color = on)
                    Spacer(Modifier.height(16.dp))
                    Button(onClick = { if (mic && model) openAccessibility(ctx) else onSetup() }) { Text(label) }
                }
            }
        }

        if (bubble != Keepalive.Bubble.Off && mic && model && !safe) {
            Spacer(Modifier.height(12.dp))
            Surface(color = MaterialTheme.colorScheme.surfaceContainerLow, shape = MaterialTheme.shapes.extraLarge) {
                Column(Modifier.padding(vertical = 12.dp)) {
                    Text(
                        "Keep it running",
                        style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurface,
                        modifier = Modifier.padding(horizontal = 20.dp, vertical = 6.dp),
                    )
                    Text(
                        "Your phone closes apps it thinks you're done with, and that switches the bubble off. Allow these so it stays.",
                        style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 20.dp),
                    )
                    KeepAliveRows(tick)
                }
            }
        }

        if (items.isNotEmpty()) {
            Spacer(Modifier.height(12.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                val words = items.sumOf { it.words }
                // Typing on a phone averages about 36 words a minute.
                val saved = (words / 36f - items.sumOf { it.audioSec.toDouble() }.toFloat() / 60f).coerceAtLeast(0f)
                Stat(words.toString(), if (words == 1) "word" else "words", Modifier.weight(1f))
                Stat(items.size.toString(), if (items.size == 1) "dictation" else "dictations", Modifier.weight(1f))
                Stat(if (saved < 1f) "<1" else saved.toInt().toString(), "min saved", Modifier.weight(1f))
            }
        }

        SectionLabel("Writing mode")
        Group {
            Row2(
                title = if (aiOn) Ai.current().name else "Exact words",
                subtitle = if (aiOn) Ai.current().blurb else "What you say, tidied up. Nothing leaves your phone.",
                onClick = onModes,
            ) { Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, null, tint = MaterialTheme.colorScheme.onSurfaceVariant) }
        }

        SectionLabel("Scratchpad")
        OutlinedTextField(
            value = scratch,
            onValueChange = { scratch = it },
            placeholder = { Text("Try it here. Nothing in this box is saved.") },
            minLines = 3,
            modifier = Modifier.fillMaxWidth(),
            shape = MaterialTheme.shapes.large,
        )
        Spacer(Modifier.height(12.dp))
        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
            InAppMic { scratch = (scratch.trimEnd() + " " + it).trim() }
        }
        if (scratch.isNotBlank()) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center) {
                TextButton(onClick = { copyText(ctx, scratch) }) { Text("Copy") }
                TextButton(onClick = { scratch = "" }) { Text("Clear") }
            }
        }
        Spacer(Modifier.height(32.dp))
    }
}

private fun greeting(): String = when (Calendar.getInstance().get(Calendar.HOUR_OF_DAY)) {
    in 5..11 -> "Morning.\nWhat's on your mind?"
    in 12..17 -> "Go ahead,\nI'm listening."
    else -> "Evening.\nSay the word."
}

@Composable
private fun Stat(value: String, label: String, modifier: Modifier) {
    Surface(color = MaterialTheme.colorScheme.surfaceContainerLow, shape = MaterialTheme.shapes.large, modifier = modifier) {
        Column(Modifier.padding(horizontal = 16.dp, vertical = 14.dp)) {
            Text(value, style = MaterialTheme.typography.headlineMedium, color = MaterialTheme.colorScheme.onSurface)
            Text(label, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

// -------------------------------------------------------------------- History

@Composable
private fun HistoryScreen() {
    val ctx = LocalContext.current
    val items by History.items.collectAsState()
    if (items.isEmpty()) {
        Column(Modifier.fillMaxSize().padding(horizontal = 20.dp)) {
            ScreenTitle("History", "Nothing yet.")
            Text(
                "Everything you dictate shows up here, so a sentence is never lost. It stays on this phone.",
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        return
    }
    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, bottom = 32.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        item { ScreenTitle("History", "Every word,\nkept safe.") }
        var lastDay = ""
        items.forEach { e ->
            val day = dayLabel(e.time)
            if (day != lastDay) {
                lastDay = day
                item(key = "day-$day") {
                    Text(
                        day,
                        style = MaterialTheme.typography.titleSmall,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(top = 10.dp, start = 4.dp).semantics { heading() },
                    )
                }
            }
            item(key = e.id) { HistoryCard(e, onCopy = { copyText(ctx, e.text) }, onDelete = { History.delete(e.id) }) }
        }
    }
}

@Composable
private fun HistoryCard(e: Entry, onCopy: () -> Unit, onDelete: () -> Unit) {
    Surface(color = MaterialTheme.colorScheme.surfaceContainerLow, shape = MaterialTheme.shapes.large, modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(start = 20.dp, end = 8.dp, top = 16.dp, bottom = 4.dp)) {
            Text(
                e.text,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.padding(end = 12.dp),
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                val meta = buildString {
                    append(SimpleDateFormat("h:mm a", Locale.getDefault()).format(Date(e.time)))
                    append("  ·  ${e.words} ${if (e.words == 1) "word" else "words"}")
                    if (e.mode != null) append("  ·  ${e.mode}")
                }
                Text(
                    meta,
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.weight(1f),
                )
                IconButton(onClick = onCopy) {
                    Icon(painterResource(R.drawable.ic_copy), "Copy", tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(20.dp))
                }
                IconButton(onClick = onDelete) {
                    Icon(Icons.Rounded.Delete, "Delete", tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(20.dp))
                }
            }
        }
    }
}

private fun dayLabel(time: Long): String {
    val day = Calendar.getInstance().apply { timeInMillis = time }
    val now = Calendar.getInstance()
    fun same(a: Calendar, b: Calendar) =
        a.get(Calendar.YEAR) == b.get(Calendar.YEAR) && a.get(Calendar.DAY_OF_YEAR) == b.get(Calendar.DAY_OF_YEAR)
    if (same(day, now)) return "Today"
    now.add(Calendar.DAY_OF_YEAR, -1)
    if (same(day, now)) return "Yesterday"
    return SimpleDateFormat("EEEE, d MMMM", Locale.getDefault()).format(Date(time))
}

// ------------------------------------------------------------------- Settings

@Composable
private fun SettingsScreen(version: Int, onChanged: () -> Unit, onModes: () -> Unit) {
    val ctx = LocalContext.current
    val tick = rememberTick()
    val bubble = tick.let { Keepalive.bubble(ctx) }
    val mic = tick.let { Dictation.micGranted(ctx) }
    val model = tick.let { Dictation.modelReady(ctx) }
    val progress by ModelDownload.progress.collectAsState()
    val upgrade = tick.let { ModelDownload.upgradeAvailable(ctx) }
    val downloadError by ModelDownload.error.collectAsState()
    val historyCount = History.items.collectAsState().value.size
    var confirmClear by remember { mutableStateOf(false) }
    var resets by remember { mutableIntStateOf(0) }
    val askMic = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok -> if (!ok) openAppInfo(ctx) }
    // Read through `version` so toggles redraw.
    val fillers = version.let { Prefs.fillers }
    val repeats = version.let { Prefs.repeats }
    val numbers = version.let { Prefs.numbers }
    val aiOn = version.let { Prefs.aiOn && Ai.configured() }

    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp)) {
        ScreenTitle("Settings", "Make it\nyours.")

        SectionLabel("Setup")
        Group {
            Row2(
                "Bubble",
                when (bubble) {
                    Keepalive.Bubble.On -> "On. It appears with your keyboard. Drag it to move it."
                    Keepalive.Bubble.Stopped -> "Stopped by your phone. Switch it off and on again in Accessibility."
                    Keepalive.Bubble.Off -> "Off. Turn it on to dictate in any app."
                },
                onClick = { openAccessibility(ctx) },
            ) {
                when (bubble) {
                    Keepalive.Bubble.On -> StatusDot(true)
                    Keepalive.Bubble.Stopped -> StatusDot(false, "Stopped")
                    Keepalive.Bubble.Off -> StatusDot(false)
                }
            }
            if ((tick + resets).let { Prefs.bubbleAt } != null) {
                Row2("Bubble position", "Moved by you. Tap to put it back by the keyboard.", onClick = {
                    Prefs.bubbleAt = null
                    resets++
                })
            }
            Row2("Microphone", if (mic) "Allowed" else "Not allowed yet",
                onClick = { if (!mic) askMic.launch(Manifest.permission.RECORD_AUDIO) else openAppInfo(ctx) }) { StatusDot(mic) }
            val p = progress
            Row2(
                "Voice model",
                when {
                    p != null -> "Downloading ${(p * 100).toInt()}%"
                    downloadError != null -> downloadError
                    upgrade -> "Older version. Tap to get the more accurate one (110 MB)."
                    model -> "Parakeet 110M, English. On this phone."
                    else -> "Not downloaded (110 MB)"
                },
                onClick = if ((!model || upgrade) && p == null) ({ ModelDownload.start(ctx) }) else null,
            ) { StatusDot(model && p == null) }
            if (p != null) {
                LinearProgressIndicator(
                    progress = { p },
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 20.dp).padding(bottom = 12.dp).height(6.dp).clip(CircleShape),
                )
            }
        }

        SectionLabel("Keep it running")
        Group { KeepAliveRows(tick) }

        SectionLabel("Clean-up")
        Group {
            SwitchRow("Drop the ums", "\"Um, so, uh, yes\" becomes \"So, yes\".", fillers) { Prefs.fillers = it; onChanged() }
            SwitchRow("Fix stutters", "\"I went went home\" becomes \"I went home\".", repeats) { Prefs.repeats = it; onChanged() }
            SwitchRow("Numbers as digits", "\"Twenty five\" becomes \"25\".", numbers) { Prefs.numbers = it; onChanged() }
        }

        SectionLabel("Writing mode")
        Group {
            Row2(
                if (aiOn) Ai.current().name else "Exact words",
                if (aiOn) "Rewritten by your own AI provider." else "Off. Add a provider to rewrite as you dictate.",
                onClick = onModes,
            ) { Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, null, tint = MaterialTheme.colorScheme.onSurfaceVariant) }
        }

        SectionLabel("Your data")
        Group {
            Row2(
                "Clear history",
                if (historyCount == 0) "Nothing saved yet." else "$historyCount saved on this phone.",
                onClick = if (historyCount > 0) ({ confirmClear = true }) else null,
            )
        }

        Spacer(Modifier.height(28.dp))
        Text(
            "Silent Voice ${remember { ctx.packageManager.getPackageInfo(ctx.packageName, 0).versionName }} for Android\nFree and open source. No account, no ads, no tracking.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(start = 4.dp, bottom = 32.dp),
        )
    }

    if (confirmClear) {
        AlertDialog(
            onDismissRequest = { confirmClear = false },
            title = { Text("Clear all history?") },
            text = { Text("This deletes $historyCount saved dictations from this phone. It can't be undone.") },
            confirmButton = { TextButton(onClick = { History.clear(); confirmClear = false }) { Text("Delete all") } },
            dismissButton = { TextButton(onClick = { confirmClear = false }) { Text("Keep them") } },
        )
    }
}

/** State shown with an icon and a label, never colour alone. */
@Composable
private fun StatusDot(ok: Boolean, problem: String = "Set up") {
    val bg = if (ok) MaterialTheme.colorScheme.tertiaryContainer else MaterialTheme.colorScheme.errorContainer
    val fg = if (ok) MaterialTheme.colorScheme.onTertiaryContainer else MaterialTheme.colorScheme.onErrorContainer
    Row(
        Modifier.clip(CircleShape).background(bg).padding(horizontal = 10.dp, vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (ok) {
            Icon(Icons.Rounded.Check, null, tint = fg, modifier = Modifier.size(14.dp))
            Spacer(Modifier.width(4.dp))
        }
        Text(if (ok) "Ready" else problem, style = MaterialTheme.typography.labelMedium, color = fg)
    }
}

/**
 * What stops the phone from closing Silent Voice in the background: battery
 * saver everywhere, plus Autostart on Xiaomi. Shared by Home, Settings and onboarding.
 */
@Composable
fun KeepAliveRows(tick: Int) {
    val ctx = LocalContext.current
    val battery = tick.let { Keepalive.batteryFree(ctx) }
    if (Keepalive.isXiaomi) {
        val auto = tick.let { Keepalive.autostart(ctx) }
        Row2(
            "Autostart",
            when (auto) {
                true -> "Allowed. The bubble comes back on its own."
                false -> "Off, so clearing Recents kills the bubble. Find Silent Voice and switch it on."
                null -> "Find Silent Voice in the list and switch it on."
            },
            onClick = { Keepalive.openAutostart(ctx) },
        ) {
            if (auto != null) StatusDot(auto, "Allow")
        }
    }
    Row2(
        "Battery",
        if (battery) "Unrestricted. Battery saver leaves the bubble alone."
        else "Battery saver may stop the bubble. Tap and choose Allow.",
        onClick = if (battery) null else ({ Keepalive.askBattery(ctx) }),
    ) { StatusDot(battery, "Allow") }
}

// ------------------------------------------------------------- Writing modes

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ModeSheet(onClose: () -> Unit) {
    var on by remember { mutableStateOf(Prefs.aiOn && Ai.configured()) }
    var mode by remember { mutableStateOf(Prefs.aiMode) }
    var baseUrl by remember { mutableStateOf(Prefs.aiBaseUrl) }
    var model by remember { mutableStateOf(Prefs.aiModel) }
    var key by remember { mutableStateOf(Prefs.apiKey) }
    val configured = baseUrl.isNotBlank() && model.isNotBlank() && key.isNotBlank()
    fun save() {
        Prefs.aiBaseUrl = baseUrl.trim()
        Prefs.aiModel = model.trim()
        Prefs.apiKey = key.trim()
        Prefs.aiMode = mode
        Prefs.aiOn = on && configured
    }

    ModalBottomSheet(onDismissRequest = { save(); onClose() }, containerColor = MaterialTheme.colorScheme.surfaceContainerLow) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 24.dp).padding(bottom = 32.dp)) {
            Text("Writing mode", style = MaterialTheme.typography.headlineSmall, color = MaterialTheme.colorScheme.onSurface)
            Spacer(Modifier.height(6.dp))
            Text(
                "Pick how your words should read. Exact words stays on your phone. The other modes send the text (never the audio) to the AI provider you choose, using your own key.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(16.dp))

            ModeRow("Exact words", "What you say, tidied up. Private and offline.", selected = !on) { on = false }
            Ai.modes.forEach { m ->
                ModeRow(m.name, m.blurb, selected = on && mode == m.id, enabled = configured) { on = true; mode = m.id }
            }

            Spacer(Modifier.height(20.dp))
            Text("AI provider", style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurface)
            Text(
                if (configured) "Connected. Your key is encrypted on this phone." else "Add a provider to unlock the modes above.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(10.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Ai.providers.take(3).forEach { p ->
                    AssistChip(
                        onClick = { baseUrl = p.baseUrl; if (model.isBlank()) model = p.modelHint },
                        label = { Text(p.name) },
                    )
                }
            }
            OutlinedTextField(
                baseUrl, { baseUrl = it }, label = { Text("Address") },
                placeholder = { Text("https://api.example.com/v1") },
                singleLine = true, modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
            )
            OutlinedTextField(
                model, { model = it }, label = { Text("Model name") },
                singleLine = true, modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
            )
            OutlinedTextField(
                key, { key = it }, label = { Text("API key") },
                singleLine = true, modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                supportingText = { Text("If the provider doesn't answer, you still get your exact words.") },
            )
            Spacer(Modifier.height(16.dp))
            FilledTonalButton(onClick = { save(); onClose() }, modifier = Modifier.fillMaxWidth().heightIn(min = 52.dp)) {
                Text("Done")
            }
        }
    }
}

@Composable
private fun ModeRow(title: String, blurb: String, selected: Boolean, enabled: Boolean = true, onPick: () -> Unit) {
    val alpha = if (enabled) 1f else 0.45f
    Row(
        Modifier.fillMaxWidth().heightIn(min = 60.dp).clip(MaterialTheme.shapes.medium)
            .background(if (selected) MaterialTheme.colorScheme.primaryContainer else Color.Transparent)
            .selectable(selected = selected, enabled = enabled, role = Role.RadioButton, onClick = onPick)
            .padding(horizontal = 12.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RadioButton(selected = selected, onClick = null, enabled = enabled)
        Column(Modifier.padding(start = 12.dp)) {
            Text(title, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurface.copy(alpha = alpha))
            Text(blurb, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = alpha))
        }
    }
}
