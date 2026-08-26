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
  "$BIN" > app.log 2>&1 &
fi
PID=$!
sleep 30

echo "--- stdout/stderr ---"; cat app.log || true
echo "--- app log ---"; cat "$LOGDIR/silent-voice.log" 2>/dev/null || echo "(no log file written)"

if ! kill -0 $PID 2>/dev/null; then
  echo "FAIL: the app exited within 30s of launch"
  exit 1
fi
kill $PID 2>/dev/null || true

if ! grep -q "Silent Voice starting" "$LOGDIR/silent-voice.log" 2>/dev/null; then
  echo "FAIL: process stayed up but never reached its own startup log line"
  exit 1
fi
echo "PASS: launches and reaches startup"

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
