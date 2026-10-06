#!/usr/bin/env bash
# Launch the packaged app and require it to reach its own startup log line.
# Packaging green only proves the binary links; this proves it runs.
set -x

if [ "$(uname -s)" = Linux ]; then
  sudo apt-get install -y ./out/*.deb
  BIN=$(ls /usr/bin/silent-voice* /usr/bin/Silent* 2>/dev/null | head -1)
  LOGDIR="$HOME/.config/SilentVoice/logs"
  rm -rf "$HOME/.config/SilentVoice"
  echo "--- installed files ---"
  dpkg -L "$(dpkg -f ./out/*.deb Package)" | head -40
  [ -n "$BIN" ] || { echo "FAIL: no executable found in /usr/bin after installing the .deb"; exit 1; }
  xvfb-run -a --server-args="-screen 0 1280x800x24" "$BIN" > app.log 2>&1 &
else
  APP=$(find src-tauri/target/release/bundle/macos -maxdepth 1 -name '*.app' | head -1)
  BIN=$(find "$APP/Contents/MacOS" -maxdepth 1 -type f | head -1)
  LOGDIR="$HOME/Library/Application Support/SilentVoice/logs"
  rm -rf "$HOME/Library/Application Support/SilentVoice"
  # Paste once, 20 s after startup, into a TextEdit window brought to the
  # front at 10 s: the v0.1.11 crash (keyboard-layout lookup off the main
  # thread) killed the app here, and v0.1.12 pasted nothing on real Macs.
  SV_SELFTEST_PASTE=20 "$BIN" > app.log 2>&1 &
fi
PID=$!
if [ "$(uname -s)" = Darwin ]; then
  sleep 10
  : > "$PWD/paste-target.txt"
  open -a TextEdit "$PWD/paste-target.txt"
  sleep 20
else
  sleep 30
fi

echo "--- stdout/stderr ---"; cat app.log || true
echo "--- app log ---"; cat "$LOGDIR/silent-voice.log" 2>/dev/null || echo "(no log file written)"

if ! kill -0 $PID 2>/dev/null; then
  echo "FAIL: the app exited within 30s of launch"
  # macOS writes a crash report; its crashing frames name the cause.
  for r in "$HOME"/Library/Logs/DiagnosticReports/silent-voice*.ips; do
    [ -f "$r" ] && grep -o '"symbol":"[^"]*"' "$r" | head -12
  done
  exit 1
fi
kill $PID 2>/dev/null || true
# Stop the app itself too ($PID is only xvfb-run on Linux); it is
# single-instance, so a leftover copy breaks the next launch test.
pkill -x 'silent-voice|AppRun.wrapped' 2>/dev/null; sleep 1; pkill -9 -x 'silent-voice|AppRun.wrapped' 2>/dev/null || true

if ! grep -q "Silent Voice starting" "$LOGDIR/silent-voice.log" 2>/dev/null; then
  echo "FAIL: process stayed up but never reached its own startup log line"
  exit 1
fi
echo "PASS: launches and reaches startup"

if [ "$(uname -s)" = Darwin ]; then
  # Public annotation (job logs need admin rights): what the paste returned.
  line=$(grep "paste from worker thread" "$LOGDIR/silent-voice.log" | tail -1)
  echo "::notice title=paste self-test::macOS $(sw_vers -productVersion): ${line:-no result logged}"
  if [ -z "$line" ]; then
    echo "FAIL: the startup paste self-test never finished"
    exit 1
  fi
  echo "PASS: pastes from a worker thread without crashing"

  # Did the text actually land at the cursor? System Events is the one app
  # CI's osascript may drive without a permission prompt.
  landed=$(osascript -e 'tell application "System Events" to tell process "TextEdit" to get value of text area 1 of scroll area 1 of window 1' 2>&1)
  echo "::notice title=paste landed in TextEdit::[$landed] | $(grep 'before paste' "$LOGDIR/silent-voice.log" | tail -1)"
  pkill -x TextEdit || true
  if [[ "$landed" != *"silent voice selftest"* ]]; then
    echo "FAIL: the paste did not land in the focused text field"
    exit 1
  fi
  echo "PASS: the text was pasted at the cursor"
fi

# Booting only proves the GUI starts. This proves the actual feature: run the
# bundled whisper CLI against a known clip and check it comes back with the
# right words. It also exercises the rpath fixes, since the CLI has to find its
# sibling libwhisper/libggml at runtime.
CLI="src-tauri/sidecars/whisper-cpp-$(rustc -vV | sed -n 's/^host: //p')"
if [ ! -x "$CLI" ]; then
  echo "FAIL: no whisper CLI sidecar for this host at $CLI"
  ls -la src-tauri/sidecars/ || true
  exit 1
fi

mkdir -p .smoke
[ -f .smoke/ggml-tiny.en.bin ] ||   curl -fsSL -o .smoke/ggml-tiny.en.bin     "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin"
[ -f .smoke/jfk.wav ] ||   curl -fsSL -o .smoke/jfk.wav     "https://github.com/ggml-org/whisper.cpp/raw/master/samples/jfk.wav"

"$CLI" -m .smoke/ggml-tiny.en.bin -f .smoke/jfk.wav --no-timestamps > stt.log 2>&1
echo "--- transcription ---"; cat stt.log

if ! grep -qi "ask not what your country" stt.log; then
  echo "FAIL: the bundled whisper build did not transcribe the sample correctly"
  exit 1
fi
echo "PASS: transcribes audio end to end"
