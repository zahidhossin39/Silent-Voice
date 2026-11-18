# Silent Voice for Android

Native Kotlin app (Jetpack Compose, Material 3). Plan and decisions:
`docs/mobile/PLAN.md`. Download: the newest `android-v*` pre-release on GitHub.

What it does: a round bubble with a mic appears above the keyboard in any app.
While listening it shows the desktop pill's bars (`Pill.kt`, ported from
`src/components/overlay/RecordingOverlay.tsx`) and an orange ring that swells
with your voice. The look was picked from a prototype kept on the branch
`prototype/bubble` (`mobile/android/prototype/bubble-prototype.html`). Tap, talk, tap again, and the
cleaned text is typed at the cursor. Drag it anywhere; the spot is remembered
(as a share of the screen, so it survives rotation) and is always kept above
the keyboard. Settings can put it back. The bars appear only once the mic is
really delivering audio (about 200 ms after the tap), so the first syllable is
not lost. Colours, icon and logo follow the desktop
app (`src/styles.css`, `app-icon.svg`). Speech is
transcribed on the phone (sherpa-onnx, Parakeet 110M with its TDT decoder; the
CTC decoder of the same model turned first words into sound-alike junk). The
model loads when the bubble is touched, not when a keyboard opens, and unloads
after 5 idle minutes. Clean-up (fillers,
repeats, numbers, "new line") is the desktop code, shared through the Rust crate
`crates/sv-core` (UniFFI). Writing modes send the text to the user's own AI
provider and fall back to the plain words on any failure.

Typing into apps: the field's text is rewritten only when it is known to be
real (`Insert.kt`, tested in `InsertTest`). An empty WhatsApp field reports its
placeholder "Message" as text with no cursor, so in that case the words are
pasted instead (they also stay on the clipboard).

Layout: `app/src/main/java/app/silentvoice/mobile/` holds `BubbleService`
(bubble + typing into the focused field), `Dictation` (the pipeline), `Stt` and
`Models` (engine, download), `Ai`, `History`, `Prefs`, and `ui/` (theme,
onboarding, screens). `uniffi/sv_core/` is generated; do not edit it.

After changing `crates/sv-core`, run `scripts\build-core.ps1`.

Check a build without speaking (prints `SELFTEST ok` to logcat tag `SVDictation`,
and the bubble's health to `SVKeepalive`). `-f 0x20000000` makes an already-open
app receive it; without it Android only brings the app to the front:

```powershell
& $adb shell am start -f 0x20000000 -n app.silentvoice.mobile/.MainActivity --ez selftest true
```

`scripts\check-bubble.ps1` kills the app the way Xiaomi does and checks the app
still knows whether the bubble is running (`$env:ANDROID_SERIAL` picks the device).

## Your words (Settings)

**Words it should recognize** (`Prefs.vocabulary`, comma or line separated) are
boosted as hotwords in Parakeet (sherpa-onnx `modified_beam_search`, score 1.5).
Only names, capitalised words, words with digits and phrases are boosted
(`Words.boosted`, tested in `WordsTest`). Plain lowercase words are not: the model
then inserts them into normal speech ("could you call me" became "Claude you call
me" at 2.5). With no boostable words, decoding stays greedy and fast. Measured on
the PC with spoken clips: "Kubirnets" became "Kubernetes", normal sentences were
unchanged at 1.5 and 2.0, cost about 0.1-0.2 s per dictation. In the app on the
emulator (self-test clips below) "Zeid ... Hasan" became "Zaid ... Hossain".

Hotwords need the model's SentencePiece `bpe.vocab`, which k2-fsa's archive lacks.
`python -I scripts/fetch_nemo_bpe_vocab.py <nemo url> <out dir>` reads only the
tokenizer out of NVIDIA's `parakeet-tdt_ctc-110m.nemo` with HTTP range requests
(about 260 KB of 460 MB; needs `pip install sentencepiece`). The result ships as
`app/src/main/assets/parakeet-tdt-110m-en.bpe.vocab`, is copied into the model
folder on first use, and matches the model's `tokens.txt` line for line. The
desktop's 0.6B `bpe.vocab` is a different tokenizer and cannot be reused.

**Shortcuts** (`Prefs.shortcuts`) are say/type pairs applied by `apply_replacements`
from `crates/sv-core` (whole words, any case). As on desktop they run after the AI
rewrite and before numbers become digits, so "n eight n" to "n8n" still sees
"eight". They fix spelled-out words that hotwords cannot.

Self-test a recording with your words and shortcuts: put a 16 kHz mono WAV in the
app's `files/selftest/` folder and add `--es clip <name>`:

```powershell
& $adb shell am start -f 0x20000000 -n app.silentvoice.mobile/.MainActivity --ez selftest true --es clip c2.wav
```

## Why the bubble can disappear

Xiaomi (MIUI / HyperOS) kills the app when it is swiped away in Recents
("SwipeUpClean" in `adb shell dumpsys activity exit-info app.silentvoice.mobile`).
Android then marks the accessibility service crashed and, without Xiaomi's
Autostart permission, does not start it again; the Accessibility switch still
shows on, and updating the app does not clear it. The app tells On / Stopped /
Off apart (`Keepalive.kt`), says how to recover (switch it off and on), and asks
for Autostart and the battery-saver exemption. Stock Android restarts the
service by itself.

## Signing

Releases are signed with this PC's debug key (`%USERPROFILE%\.android\debug.keystore`).
A build signed elsewhere will not install over it; keep that file, or uninstall first.

## Build and run (PowerShell)

```powershell
# Once: fetch the sherpa-onnx library at the desktop's pinned version
.\scripts\fetch-sherpa.ps1

# Gradle fails on this PC with "Unable to establish loopback connection" unless
# Java is given a short folder for its sockets. Set this in every new terminal.
$env:JAVA_TOOL_OPTIONS = "-Djdk.net.unixdomain.tmpdir=D:/svtmp"

.\gradlew.bat assembleDebug
& "$env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe" install -r app\build\outputs\apk\debug\app-debug.apk
```

`local.properties` (not committed) must hold `sdk.dir=` pointing at the Android SDK.

## Results on the test phone

Xiaomi 21091116UI (Dimensity 920, 8 cores, 6 GB RAM, Android 13), 4 threads,
2026-11-12. Same 16.7 s clip, two runs each.

| Model | Download | Load | 16.7 s clip | Speed vs real time |
|---|---|---|---|---|
| Parakeet 110M | 100 MB | 1.3-2.4 s | 0.8 s (1.2-1.3 s right after the big downloads) | 13-21x |
| Moonshine Tiny | 103 MB | 1.9 s | 1.4 s | 12x |
| Moonshine Base | 240 MB | 3.5 s | 3.7 s | 4.5x |

All three transcribed the clip correctly. **Default: Parakeet 110M**: fastest,
smallest download, and the only one near the 1 s target. It is not reliably under
1 s: the same clip took 0.8 s on a rested phone and 1.2-1.3 s when the phone had
just finished downloading and unpacking 440 MB. Not yet measured: real speech
through the microphone, noisy rooms, accents, battery and heat over long use.

## Emulator results

Emulator on the dev PC (x86_64, 4 threads), 2026-11-12. These are PC-CPU numbers,
not phone numbers; they only prove the app works.

| Model | Download | Load | 16.7 s clip, cold | warm |
|---|---|---|---|---|
| Moonshine Tiny | 103 MB | 2.4 s | 2.2 s | 1.6 s |
| Moonshine Base | 240 MB | 8.0 s | 4.5 s | 2.9 s |
| Parakeet 110M | 100 MB | 2.8 s | 1.1 s | 1.6 s |

All three got the sample right apart from one word in Parakeet ("dishonoud").
