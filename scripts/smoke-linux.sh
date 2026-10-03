#!/usr/bin/env bash
# Launch Silent Voice and require it to reach its own startup log line.
# Usage: bash scripts/smoke-linux.sh path/to/Silent.Voice.AppImage
#        bash scripts/smoke-linux.sh /usr/bin/silent-voice      (installed package)
# Runs on any distro with xvfb-run; used by CI on Arch, Fedora and Ubuntu.
set -x
APP="$(readlink -f "$1")"
chmod +x "$APP" 2>/dev/null || true
LOGDIR="$HOME/.config/SilentVoice/logs"
rm -rf "$HOME/.config/SilentVoice"

# Containers have no FUSE; extract-and-run is what a user without libfuse2
# gets too, so it exercises the same bundled libraries.
APPIMAGE_EXTRACT_AND_RUN=1 xvfb-run -a --server-args="-screen 0 1280x800x24" \
  "$APP" > app.log 2>&1 &
PID=$!
sleep 40

echo "--- stdout/stderr ---"; cat app.log || true

# Failure details as a CI annotation: job logs need repo-admin rights to read,
# annotations are public, so whoever debugs a failure can see the cause.
fail() {
  echo "FAIL: $1"
  if [ -n "$GITHUB_ACTIONS" ]; then
    body=$( { echo "$1"; echo "--- stderr ---"; tail -25 app.log; echo "--- app log ---"; tail -15 "$LOGDIR/silent-voice.log" 2>/dev/null; } | sed 's/%/%25/g' | awk '{printf "%s%%0A", $0}')
    echo "::error title=smoke-linux::$body"
  fi
  exit 1
}
echo "--- app log ---"; cat "$LOGDIR/silent-voice.log" 2>/dev/null || echo "(no log file written)"

if ! kill -0 $PID 2>/dev/null; then
  fail "the app exited within 40s of launch"
fi
kill $PID 2>/dev/null || true
if ! grep -q "Silent Voice starting" "$LOGDIR/silent-voice.log" 2>/dev/null; then
  fail "process stayed up but never reached its own startup log line"
fi
if grep -qiE "EGL|Failed to create GBM|cannot open shared object|symbol lookup error" app.log; then
  fail "started, but with graphics/library errors (likely a blank window)"
fi
echo "PASS: launches and reaches startup"
