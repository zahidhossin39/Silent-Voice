#!/usr/bin/env bash
# Populate src-tauri/sidecars/ with the bundled binaries for the HOST platform.
# Windows sidecars are committed to the repo; macOS and Linux are fetched here
# because their upstream builds are large and platform-specific.
#
# Only whisper is fatal. llama / piper / sherpa back optional features, so a
# failed fetch leaves an empty directory and the app starts without them
# rather than failing the whole build.
set -euo pipefail

WHISPER_TAG=b4938
LLAMA_TAG=b10631
PIPER_TAG=2023.11.14-2
SHERPA_TAG=v1.13.6

OS=$(uname -s)
TRIPLE=$(rustc -vV | sed -n 's/^host: //p')
ROOT=$(cd "$(dirname "$0")/.." && pwd)
SIDECARS="$ROOT/src-tauri/sidecars"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

mkdir -p "$SIDECARS"/{llama,piper,sherpa}

get() { curl -fsSL "$1" -o "$2"; }

# Anything under sidecars/ must find its siblings at runtime, since Tauri
# flattens them next to the app executable.
fix_rpath() {
  local dir=$1
  if [ "$OS" = Darwin ]; then
    find "$dir" -maxdepth 1 -type f -perm -u+x ! -name '*.dylib' \
      -exec install_name_tool -add_rpath @loader_path {} \; 2>/dev/null || true
  else
    command -v patchelf >/dev/null || return 0
    find "$dir" -maxdepth 1 -type f -perm -u+x ! -name '*.so*' \
      -exec patchelf --set-rpath '$ORIGIN' {} \; 2>/dev/null || true
  fi
}

echo "==> whisper.cpp"
if [ "$OS" = Darwin ]; then
  # No macOS release assets upstream, so build it. Metal is on by default.
  git clone --depth 1 --branch "$WHISPER_TAG" https://github.com/ggml-org/whisper.cpp "$TMP/w"
  cmake -S "$TMP/w" -B "$TMP/w/build" \
    -DCMAKE_BUILD_TYPE=Release \
    -DBUILD_SHARED_LIBS=ON \
    -DCMAKE_BUILD_WITH_INSTALL_RPATH=ON \
    -DCMAKE_INSTALL_RPATH=@loader_path
  cmake --build "$TMP/w/build" --config Release -j"$(sysctl -n hw.ncpu)"
  BIN="$TMP/w/build/bin"
else
  get "https://github.com/ggml-org/whisper.cpp/releases/download/$WHISPER_TAG/whisper-bin-ubuntu-x64.tar.gz" "$TMP/w.tgz"
  tar xzf "$TMP/w.tgz" -C "$TMP"
  BIN="$TMP/whisper-bin-ubuntu-x64"
fi

cp "$BIN/whisper-server" "$SIDECARS/whisper-server"
# externalBin resolves by target triple, and the CLI is what that sidecar runs.
cp "$BIN/whisper-cli" "$SIDECARS/whisper-cpp-$TRIPLE"
find "$BIN" -maxdepth 1 \( -name '*.dylib' -o -name '*.so' -o -name '*.so.*' \) -exec cp {} "$SIDECARS/" \;
chmod +x "$SIDECARS/whisper-server" "$SIDECARS/whisper-cpp-$TRIPLE"
fix_rpath "$SIDECARS"

optional() {
  local name=$1 url=$2 dest=$3 strip=$4
  echo "==> $name"
  local f="$TMP/$name.archive"
  if ! get "$url" "$f"; then
    echo "    SKIPPED: $url did not download — $name features will be absent."
    return 0
  fi
  mkdir -p "$dest"
  case "$url" in
    *.tar.bz2) tar xjf "$f" -C "$dest" --strip-components="$strip" ;;
    *.tar.gz)  tar xzf "$f" -C "$dest" --strip-components="$strip" ;;
    *.zip)     unzip -qo "$f" -d "$dest" ;;
  esac
  chmod -R u+rwX "$dest"
  fix_rpath "$dest"
}

if [ "$OS" = Darwin ]; then
  LLAMA_ASSET=llama-$LLAMA_TAG-bin-macos-arm64.tar.gz
  PIPER_ASSET=piper_macos_aarch64.tar.gz
  SHERPA_ASSET=sherpa-onnx-$SHERPA_TAG-osx-arm64-shared.tar.bz2
else
  LLAMA_ASSET=llama-$LLAMA_TAG-bin-ubuntu-x64.tar.gz
  PIPER_ASSET=piper_linux_x86_64.tar.gz
  SHERPA_ASSET=sherpa-onnx-$SHERPA_TAG-linux-x64-shared.tar.bz2
fi

optional llama "https://github.com/ggml-org/llama.cpp/releases/download/$LLAMA_TAG/$LLAMA_ASSET" "$SIDECARS/llama" 1
optional piper "https://github.com/rhasspy/piper/releases/download/$PIPER_TAG/$PIPER_ASSET" "$SIDECARS/piper" 1
optional sherpa "https://github.com/k2-fsa/sherpa-onnx/releases/download/$SHERPA_TAG/$SHERPA_ASSET" "$TMP/sherpa" 1

# sherpa ships headers, CLIs and libs in one tree; the app only loads two libs.
find "$TMP/sherpa" -name 'libsherpa-onnx-c-api.*' -o -name 'libonnxruntime.*' \
  | while read -r f; do cp "$f" "$SIDECARS/sherpa/"; done
fix_rpath "$SIDECARS/sherpa"

echo
echo "==> sidecars/"
ls -la "$SIDECARS"
for d in llama piper sherpa; do echo "--- $d"; ls "$SIDECARS/$d" | head -20; done
