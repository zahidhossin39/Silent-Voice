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
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.Edit
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
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
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
import androidx.compose.runtime.rememberCoroutineScope
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
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import app.silentvoice.mobile.Ai
import app.silentvoice.mobile.Dictation
import app.silentvoice.mobile.Entry
import app.silentvoice.mobile.History
import app.silentvoice.mobile.Keepalive
import app.silentvoice.mobile.Engine
import app.silentvoice.mobile.Words
import app.silentvoice.mobile.ModelDownload
import app.silentvoice.mobile.Prefs
import app.silentvoice.mobile.R
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
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
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    // Delete at once, offer Undo for a few seconds. Lives here so leaving History doesn't cancel it.
    fun deleted(e: Entry) {
        scope.launch {
            snackbar.currentSnackbarData?.dismiss()
            val result = withTimeoutOrNull(5_000) {
                snackbar.showSnackbar("Dictation deleted", actionLabel = "Undo", duration = SnackbarDuration.Indefinite)
            }
            if (result == SnackbarResult.ActionPerformed) History.restore(e)
        }
    }

    Scaffold(
        containerColor = MaterialTheme.colorScheme.surface,
        snackbarHost = { SnackbarHost(snackbar) },
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
                Tab.History -> HistoryScreen(onDeleted = ::deleted)
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

/** A section heading, with an optional one-line "what this is for" under it. */
@Composable
private fun SectionLabel(text: String, hint: String? = null) {
    Text(
        text,
        style = MaterialTheme.typography.titleSmall,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(top = 28.dp, bottom = if (hint == null) 8.dp else 2.dp, start = 4.dp).semantics { heading() },
    )
    if (hint != null) {
        Text(
            hint,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(start = 4.dp, end = 4.dp, bottom = 10.dp),
        )
    }
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
                subtitle = if (aiOn) Ai.current().blurb else EXACT_BLURB,
                onClick = onModes,
            ) { Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, null, tint = MaterialTheme.colorScheme.onSurfaceVariant) }
        }

        SectionLabel("Scratchpad")
        OutlinedTextField(
            value = scratch,
            onValueChange = { scratch = it },
            placeholder = { Text("Try it here. What you say is also saved in History.") },
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
private fun HistoryScreen(onDeleted: (Entry) -> Unit) {
    val ctx = LocalContext.current
    val items by History.items.collectAsState()
    var editingId by rememberSaveable { mutableStateOf<Long?>(null) }
    val editing = editingId?.let { id -> items.firstOrNull { it.id == id } }
    if (editing != null) HistoryEditSheet(editing, onClose = { editingId = null })
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
            item(key = e.id) {
                HistoryCard(
                    e,
                    onEdit = { editingId = e.id },
                    onCopy = { copyText(ctx, e.text) },
                    onDelete = { History.delete(e.id); onDeleted(e) },
                )
            }
        }
    }
}

@Composable
private fun HistoryCard(e: Entry, onEdit: () -> Unit, onCopy: () -> Unit, onDelete: () -> Unit) {
    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerLow,
        shape = MaterialTheme.shapes.large,
        modifier = Modifier.fillMaxWidth().clip(MaterialTheme.shapes.large)
            .clickable(onClickLabel = "Edit dictation", role = Role.Button, onClick = onEdit),
    ) {
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

/** Fix a past dictation's text. Its time and mode stay as they were. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun HistoryEditSheet(e: Entry, onClose: () -> Unit) {
    var text by rememberSaveable(e.id) { mutableStateOf(e.text) }
    ModalBottomSheet(sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true), onDismissRequest = onClose, containerColor = MaterialTheme.colorScheme.surfaceContainerLow) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 24.dp).padding(bottom = 32.dp)) {
            Text("Edit dictation", style = MaterialTheme.typography.headlineSmall, color = MaterialTheme.colorScheme.onSurface)
            Spacer(Modifier.height(6.dp))
            Text(
                "Fix anything it misheard. The change is saved on this phone only.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            OutlinedTextField(
                text, { text = it },
                label = { Text("Text") },
                minLines = 3,
                modifier = Modifier.fillMaxWidth().padding(top = 16.dp),
            )
            Spacer(Modifier.height(16.dp))
            FilledTonalButton(
                onClick = { History.update(e.id, text.trim()); onClose() },
                enabled = text.isNotBlank(),
                modifier = Modifier.fillMaxWidth().heightIn(min = 52.dp),
            ) { Text("Save") }
            TextButton(onClick = onClose, modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp)) { Text("Cancel") }
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
    var wordsSheet by rememberSaveable { mutableStateOf(false) }
    var shortcutsSheet by rememberSaveable { mutableStateOf(false) }
    var words by remember { mutableIntStateOf(0) } // bumped when either sheet saves
    val askMic = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok -> if (!ok) openAppInfo(ctx) }
    // Read through `version` so toggles redraw.
    val fillers = version.let { Prefs.fillers }
    val repeats = version.let { Prefs.repeats }
    val numbers = version.let { Prefs.numbers }
    val aiOn = version.let { Prefs.aiOn && Ai.configured() }

    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp)) {
        ScreenTitle("Settings", "Make it\nyours.")

        SectionLabel("Setup", "What it needs so you can talk instead of type in any app.")
        Group {
            Row2(
                "Bubble",
                when (bubble) {
                    Keepalive.Bubble.On -> "On. A mic bubble shows above your keyboard. Tap it, talk, tap again. Drag it to move it."
                    Keepalive.Bubble.Stopped -> "Stopped by your phone. Switch it off and on again in Accessibility."
                    Keepalive.Bubble.Off -> "Off. Turn it on to get a mic bubble above your keyboard in every app."
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
                Row2("Bubble position", "You moved it. Tap to put it back above the keyboard.", onClick = {
                    Prefs.bubbleAt = null
                    resets++
                })
            }
            Row2("Microphone", if (mic) "Allowed, so it can hear you." else "Not allowed yet. It can't hear you until you allow it.",
                onClick = { if (!mic) askMic.launch(Manifest.permission.RECORD_AUDIO) else openAppInfo(ctx) }) { StatusDot(mic) }
            val p = progress
            Row2(
                "Voice model",
                when {
                    p != null -> "Downloading ${(p * 100).toInt()}%"
                    downloadError != null -> downloadError
                    upgrade -> "Older version. Tap to get the more accurate one (110 MB)."
                    model -> "Turns speech into text on this phone, offline. Parakeet, English."
                    else -> "Turns your speech into text. Tap to download it (110 MB)."
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

        SectionLabel("Keep it running", "Phones close apps to save battery. These stop that from switching off the bubble.")
        Group { KeepAliveRows(tick) }

        SectionLabel("Clean-up", "Small fixes to your words before they're typed. Done on this phone.")
        Group {
            SwitchRow("Drop the ums", "“Um, so, uh, yes” becomes “So, yes”.", fillers) { Prefs.fillers = it; onChanged() }
            SwitchRow("Fix stutters", "“I went went home” becomes “I went home”.", repeats) { Prefs.repeats = it; onChanged() }
            SwitchRow("Numbers as digits", "“Twenty five” becomes “25”.", numbers) { Prefs.numbers = it; onChanged() }
        }

        SectionLabel("Your words", "Help it get your names right, and save phrases that type longer text.")
        Group {
            val vocab = (version + words).let { Words.entries(Prefs.vocabulary) }
            Row2(
                "Words it should recognize",
                if (vocab.isEmpty()) "Names and terms it spells wrong. Add “Zaid” and it stops typing “Said”."
                else "Spells these right: " + vocab.take(3).joinToString(", ") +
                    if (vocab.size > 3) " and ${vocab.size - 3} more." else ".",
                onClick = { wordsSheet = true },
            ) { AddIcon() }
            val shortcuts = (version + words).let { Prefs.shortcuts }
            Row2(
                "Shortcuts",
                if (shortcuts.isEmpty()) "Say a short phrase to type longer text. “My email” types your address."
                else {
                    val first = shortcuts.first()
                    val more = shortcuts.size - 1
                    "“${first.say}” → ${preview(first.type)}" +
                        if (more > 0) " and $more more." else ""
                },
                onClick = { shortcutsSheet = true },
            ) { AddIcon() }
        }

        SectionLabel("Writing mode", "Type exactly what you said, or let an AI rewrite it first.")
        Group {
            Row2(
                if (aiOn) Ai.current().name else "Exact words",
                if (aiOn) Ai.current().blurb + " Uses your own AI provider." else "$EXACT_BLURB Tap to add an AI that rewrites it.",
                onClick = onModes,
            ) { AddIcon() }
        }

        SectionLabel("Your data", "Past dictations are kept only on this phone, for the History tab.")
        Group {
            Row2(
                "Clear history",
                when (historyCount) {
                    0 -> "Nothing saved yet."
                    1 -> "1 dictation saved. Tap to delete it."
                    else -> "$historyCount dictations saved. Tap to delete them all."
                },
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

    if (wordsSheet) WordsSheet(onClose = { wordsSheet = false; words++ })
    if (shortcutsSheet) ShortcutsSheet(onClose = { shortcutsSheet = false; words++ })

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

/** What "Exact words" does, said the same way everywhere. */
private const val EXACT_BLURB = "Your own words, cleaned of ums and repeats. Stays on this phone."

/** A shortcut's typed text, cut short for a one-line preview. */
private fun preview(type: String): String {
    val oneLine = type.replace(Regex("\\s+"), " ").trim()
    return when {
        oneLine.isEmpty() -> "(removed)"
        oneLine.length > 24 -> oneLine.take(23).trimEnd() + "…"
        else -> oneLine
    }
}

/** Trailing icon for a Settings row whose page is where you add things. */
@Composable
private fun AddIcon() = Icon(Icons.Rounded.Add, null, tint = MaterialTheme.colorScheme.primary)

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
        if (battery) "Unrestricted, so battery saver leaves the bubble alone."
        else "Battery saver may switch off the bubble. Tap and choose Allow.",
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
    var showKey by remember { mutableStateOf(false) }
    val configured = baseUrl.isNotBlank() && model.isNotBlank() && key.isNotBlank()
    fun save() {
        Prefs.aiBaseUrl = baseUrl.trim()
        Prefs.aiModel = model.trim()
        Prefs.apiKey = key.trim()
        Prefs.aiMode = mode
        Prefs.aiOn = on && configured
    }

    ModalBottomSheet(sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true), onDismissRequest = { save(); onClose() }, containerColor = MaterialTheme.colorScheme.surfaceContainerLow) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 24.dp).padding(bottom = 32.dp)) {
            Text("Writing mode", style = MaterialTheme.typography.headlineSmall, color = MaterialTheme.colorScheme.onSurface)
            Spacer(Modifier.height(6.dp))
            Text(
                "Pick how your words should read. Exact words stays on your phone. The other modes send the text (never the audio) to the AI provider you choose, using your own key.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(16.dp))

            ModeRow("Exact words", EXACT_BLURB, selected = !on) { on = false }
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
                        onClick = {
                            baseUrl = p.baseUrl
                            // Swap in this provider's model unless the user typed their own;
                            // keeping OpenAI's model on Groq would fail every request.
                            if (model.isBlank() || Ai.providers.any { it.modelHint == model.trim() }) model = p.modelHint
                        },
                        label = { Text(p.name) },
                    )
                }
            }
            OutlinedTextField(
                baseUrl, { baseUrl = it }, label = { Text("Server address") },
                placeholder = { Text("https://api.example.com/v1") },
                singleLine = true, modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                keyboardOptions = KeyboardOptions(
                    keyboardType = KeyboardType.Uri,
                    autoCorrectEnabled = false,
                    capitalization = KeyboardCapitalization.None,
                ),
                supportingText = { Text("Filled in when you pick a provider above.") },
            )
            OutlinedTextField(
                model, { model = it }, label = { Text("Model name") },
                singleLine = true, modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
            )
            OutlinedTextField(
                key, { key = it }, label = { Text("API key") },
                singleLine = true, modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                visualTransformation = if (showKey) VisualTransformation.None else PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, autoCorrectEnabled = false),
                trailingIcon = {
                    TextButton(onClick = { showKey = !showKey }) { Text(if (showKey) "Hide" else "Show") }
                },
                supportingText = { Text("Get a key from your provider's website. If the provider doesn't answer, you still get your exact words.") },
            )
            if (configured) {
                TextButton(
                    onClick = { baseUrl = ""; model = ""; key = ""; on = false },
                    modifier = Modifier.heightIn(min = 48.dp),
                ) { Text("Remove provider") }
            }
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

// ---------------------------------------------------------------- Your words

/** The list of words the voice model should favor (see [Words.boosted]). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun WordsSheet(onClose: () -> Unit) {
    val ctx = LocalContext.current
    var text by rememberSaveable { mutableStateOf(Prefs.vocabulary) }
    val entries = Words.entries(text)
    val boosted = Words.boosted(text)
    val skipped = entries.filter { e -> boosted.none { it.equals(e, ignoreCase = true) } }
        .distinctBy { it.lowercase() }
    val canBias = remember { Dictation.model(ctx).engine == Engine.NEMO_TDT }
    fun save() { Prefs.vocabulary = text.trim() }

    ModalBottomSheet(sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true), onDismissRequest = { save(); onClose() }, containerColor = MaterialTheme.colorScheme.surfaceContainerLow) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 24.dp).padding(bottom = 32.dp)) {
            Text("Words it should recognize", style = MaterialTheme.typography.headlineSmall, color = MaterialTheme.colorScheme.onSurface)
            Spacer(Modifier.height(6.dp))
            Text(
                "Names and terms it keeps spelling wrong. Write each one the way it should look, separated by commas. Add “Zaid” and it stops typing “Said”.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            OutlinedTextField(
                text, { text = it },
                label = { Text("Your words") },
                placeholder = { Text("Zaid, Kubernetes, Claude Code") },
                minLines = 3,
                // Names aren't dictionary words: autocorrect would "fix" Zaid to Said.
                keyboardOptions = KeyboardOptions(autoCorrectEnabled = false),
                modifier = Modifier.fillMaxWidth().padding(top = 16.dp),
                supportingText = {
                    Text(
                        when {
                            !canBias -> "Starts working once the voice model update is installed (Settings > Voice model)."
                            skipped.isNotEmpty() -> "Skipped: ${skipped.joinToString(", ")}. Plain lowercase words would start popping up in normal speech, so capitalize names."
                            boosted.isNotEmpty() -> "Listening for ${boosted.size} ${if (boosted.size == 1) "word" else "words"}. Dictation takes a moment longer while this list has words."
                            else -> "Everything stays on this phone."
                        },
                    )
                },
            )
            Spacer(Modifier.height(8.dp))
            Text(
                "Want one phrase to type something else, like \u201cn eight n\u201d typing n8n? Use Shortcuts.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(16.dp))
            FilledTonalButton(onClick = { save(); onClose() }, modifier = Modifier.fillMaxWidth().heightIn(min = 52.dp)) {
                Text("Done")
            }
        }
    }
}

/** "Say this, type that" pairs, applied to every dictation. Tap one to edit it in place. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ShortcutsSheet(onClose: () -> Unit) {
    var list by remember { mutableStateOf(Prefs.shortcuts) }
    var say by rememberSaveable { mutableStateOf("") }
    var type by rememberSaveable { mutableStateOf("") }
    // Index of the shortcut loaded into the form, or -1 when adding a new one.
    var editing by rememberSaveable { mutableIntStateOf(-1) }
    if (editing >= list.size) editing = -1
    val scroll = rememberScrollState()
    val scope = rememberCoroutineScope()
    fun store(new: List<Words.Shortcut>) { list = new; Prefs.shortcuts = new }
    // The entry being edited doesn't count, so you can change only its "Type".
    val clash = list.withIndex().any { (i, it) -> i != editing && it.say.equals(say.trim(), ignoreCase = true) }
    val canSave = say.isNotBlank() && !clash
    fun resetForm() { say = ""; type = ""; editing = -1 }
    // Add the form as a new shortcut, or replace the one being edited at the same position.
    fun commit() {
        if (!canSave) return
        val sc = Words.Shortcut(say.trim(), type)
        store(if (editing >= 0) list.toMutableList().also { it[editing] = sc } else list + sc)
        resetForm()
    }
    // Switching or closing keeps a finished shortcut rather than dropping it. One
    // with an empty "Type" (removes the phrase) is only saved with the button, so
    // a half-typed form never quietly becomes a "delete this word" rule.
    fun autoSave() { if (type.isNotBlank()) commit() }
    fun startEdit(i: Int) {
        if (i == editing) return
        autoSave() // keep a finished shortcut typed in the form (adds append, so i stays valid)
        val sc = list[i]
        say = sc.say
        type = sc.type
        editing = i
        // The form sits at the top, above the keyboard; bring it into view.
        scope.launch { scroll.animateScrollTo(0) }
    }
    fun delete(i: Int) {
        store(list.toMutableList().also { it.removeAt(i) })
        when {
            i == editing -> resetForm()
            i < editing -> editing--
        }
    }
    fun close() {
        autoSave()
        onClose()
    }

    ModalBottomSheet(sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true), onDismissRequest = ::close, containerColor = MaterialTheme.colorScheme.surfaceContainerLow) {
        Column(Modifier.verticalScroll(scroll).padding(horizontal = 24.dp).padding(bottom = 32.dp)) {
            Text("Shortcuts", style = MaterialTheme.typography.headlineSmall, color = MaterialTheme.colorScheme.onSurface)
            Spacer(Modifier.height(6.dp))
            Text(
                "Say a short phrase and longer text is typed instead. Say “my email” and your address is typed. Matches whole words, any capitals.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(16.dp))
            Text(
                if (editing >= 0) "Edit shortcut" else "Add one",
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.semantics { heading() },
            )
            OutlinedTextField(
                say, { say = it }, label = { Text("When I say") }, placeholder = { Text("my email") },
                singleLine = true, modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                keyboardOptions = KeyboardOptions(autoCorrectEnabled = false),
                isError = clash,
                supportingText = if (clash) ({ Text("You already have this one. Tap it in the list to change it.") }) else null,
            )
            OutlinedTextField(
                type, { type = it }, label = { Text("Type") }, placeholder = { Text("name@example.com") },
                modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                keyboardOptions = KeyboardOptions(autoCorrectEnabled = false),
                supportingText = { Text("Leave empty to remove the phrase from your text.") },
            )
            Spacer(Modifier.height(16.dp))
            FilledTonalButton(
                onClick = ::commit,
                enabled = canSave,
                modifier = Modifier.fillMaxWidth().heightIn(min = 52.dp),
            ) { Text(if (editing >= 0) "Save changes" else "Add shortcut") }
            if (editing >= 0) {
                TextButton(onClick = ::resetForm, modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp)) { Text("Cancel") }
            } else {
                TextButton(onClick = ::close, modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp)) { Text("Done") }
            }
            Spacer(Modifier.height(24.dp))
            if (list.isNotEmpty()) {
                Text(
                    "Your shortcuts",
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier.semantics { heading() },
                )
                Text(
                    "Tap one to change it.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(bottom = 4.dp),
                )
            }
            list.forEachIndexed { i, sc ->
                val selected = i == editing
                Row(
                    Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(vertical = 2.dp)
                        .clip(MaterialTheme.shapes.medium)
                        .background(if (selected) MaterialTheme.colorScheme.primaryContainer else Color.Transparent)
                        .clickable(onClickLabel = "Edit", role = Role.Button) { startEdit(i) }
                        .semantics {
                            contentDescription = "Edit shortcut ${sc.say}. Types ${sc.type.ifEmpty { "nothing" }}" +
                                if (selected) ". Editing now" else ""
                        }
                        .padding(start = 12.dp, top = 6.dp, bottom = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text("“${sc.say}”", style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurface)
                        Text(
                            "types " + sc.type.ifEmpty { "nothing (the phrase is removed)" },
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 2,
                        )
                    }
                    Icon(
                        Icons.Rounded.Edit, null,
                        tint = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.size(20.dp),
                    )
                    IconButton(onClick = { delete(i) }) {
                        Icon(Icons.Rounded.Delete, "Delete shortcut ${sc.say}", tint = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
            if (list.isEmpty()) {
                Text(
                    "None yet. Your shortcuts will show here.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}
