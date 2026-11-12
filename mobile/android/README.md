# Silent Voice for Android

Native Kotlin app. Plan and decisions: `docs/mobile/PLAN.md`. Right now this is
the step 1 engine spike: it downloads a small English model, records or uses a
sample clip, and shows the text and how long it took.

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

## Measure the three models

With a phone connected over USB (USB debugging on), or the `sv_test` emulator running:

```powershell
$adb = "$env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe"
& $adb logcat -c
& $adb shell am start -n app.silentvoice.mobile/.MainActivity --es bench all
& $adb logcat -s SVBench:*      # ends with "BENCH DONE"
```

Each model runs the same 16.7 s sample clip twice (cold, then warm). The first
run downloads about 440 MB of models inside the app.

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
