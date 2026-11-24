#!/bin/bash
# Install or update Silent Voice on a Mac with one command:
#
#   curl -fsSL https://raw.githubusercontent.com/zahidhossin39/Silent-Voice/main/scripts/install-macos.sh | bash
#
# Why this exists: browsers tag downloaded files as "quarantined", and macOS
# then refuses apps without Apple's paid Developer ID ("Apple could not verify
# ... is free of malware"). Files that curl downloads get no quarantine tag, so
# this installs the same release build with no Gatekeeper prompt. The in-app
# updater downloads the same way, so later updates need nothing extra.
#
# Options (environment variables):
#   SV_INSTALL_DIR   where the app goes (default /Applications)
#   SV_NO_OPEN=1     do not open the app after installing (used by CI)
#   SV_FEED          updater feed URL (default: the latest GitHub release)

# Everything runs inside main(), called on the last line. If the download of
# this script is cut off halfway, bash never reaches that line and runs nothing.
set -euo pipefail

REPO="zahidhossin39/Silent-Voice"
APP_NAME="Silent Voice.app"
# The project's own self-signed certificate (scripts/macos-signing.sh). Every
# release is signed with it, so a mismatch means a broken or foreign download.
SIGNING_IDENTITY="Silent Voice Self-Signed"
MIN_MACOS_MAJOR=12

say()  { printf '\033[1;38;5;208m==>\033[0m %s\n' "$*"; }
fail() { printf '\033[1;31mError:\033[0m %s\n' "$*" >&2; exit 1; }

main() {
  local install_dir="${SV_INSTALL_DIR:-/Applications}"
  local feed="${SV_FEED:-https://github.com/$REPO/releases/latest/download/latest.json}"

  [ "$(uname -s)" = "Darwin" ] || fail "This installer is for macOS. Windows and Linux installers are on https://github.com/$REPO/releases"
  [ "$(uname -m)" = "arm64" ] || fail "Silent Voice for Mac needs an Apple Silicon Mac (M1 or newer). Intel Macs are not supported yet."
  local major
  major="$(sw_vers -productVersion | cut -d. -f1)"
  [ "$major" -ge "$MIN_MACOS_MAJOR" ] || fail "Silent Voice needs macOS $MIN_MACOS_MAJOR or newer (this Mac has $(sw_vers -productVersion))."

  local tmp
  tmp="$(mktemp -d)"
  # shellcheck disable=SC2064  # expand $tmp now, while it is set
  trap "rm -rf '$tmp'" EXIT

  say "Finding the latest version"
  local json url version
  json="$(curl -fsSL --retry 3 "$feed")" || fail "Could not reach GitHub. Check the internet connection and try again."
  # The updater feed lists one URL per platform; take the Apple Silicon one.
  url="$(printf '%s' "$json" | tr -d '\n' | grep -o '"darwin-aarch64"[^}]*' | grep -o 'https://[^"]*' | head -n 1 || true)"
  version="$(printf '%s' "$json" | grep -o '"version"[^,]*' | head -n 1 | grep -o '[0-9][0-9.]*' || true)"
  [ -n "$url" ] || fail "The latest release has no Mac build yet. Try again later."

  say "Downloading Silent Voice ${version:-}"
  curl -fL --retry 3 --progress-bar -o "$tmp/app.tar.gz" "$url" || fail "Download failed. Try again."
  tar -xzf "$tmp/app.tar.gz" -C "$tmp" || fail "The download is damaged. Try again."
  [ -d "$tmp/$APP_NAME" ] || fail "The download does not contain $APP_NAME."

  say "Checking the app's signature"
  codesign --verify --deep --strict "$tmp/$APP_NAME" 2>/dev/null \
    || fail "The app's signature is broken. Not installing it."
  codesign -dvv "$tmp/$APP_NAME" 2>&1 | grep -q "Authority=$SIGNING_IDENTITY" \
    || fail "The app is not signed by Silent Voice. Not installing it."

  if pgrep -xq "silent-voice" || pgrep -fq "$APP_NAME/Contents/MacOS"; then
    say "Closing the running Silent Voice"
    osascript -e 'quit app "Silent Voice"' >/dev/null 2>&1 || true
    sleep 2
    pkill -f "$APP_NAME/Contents/MacOS" 2>/dev/null || true
  fi

  say "Installing to $install_dir"
  local dest="$install_dir/$APP_NAME"
  # sudo only when the folder is not writable (standard, non-admin accounts).
  local sudo=""
  if [ ! -w "$install_dir" ] || { [ -e "$dest" ] && [ ! -w "$dest" ]; }; then
    say "Your Mac password is needed to write to $install_dir"
    sudo="sudo"
  fi
  $sudo mkdir -p "$install_dir"
  $sudo rm -rf "$dest"
  # ditto keeps the code signature, symlinks and extended attributes intact.
  $sudo ditto "$tmp/$APP_NAME" "$dest"
  # curl adds no quarantine tag; this clears one left by an earlier .dmg install.
  $sudo xattr -dr com.apple.quarantine "$dest" 2>/dev/null || true

  say "Silent Voice ${version:-} is installed."
  if [ "${SV_NO_OPEN:-0}" != "1" ]; then
    open "$dest"
    say "On first launch, allow Microphone and Accessibility when macOS asks."
  fi
}

main "$@"
