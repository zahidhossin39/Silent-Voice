#!/usr/bin/env bash
# Make the bundled libwayland-* a fallback instead of the default, in place.
# Usage: bash scripts/fix-appimage.sh path/to/app.AppImage
#
# Why: the AppImage is built on Ubuntu 22.04 and bundles that release's
# libwayland, but it always uses the HOST's Mesa EGL driver. On newer distros
# (Arch, Fedora) that Mesa needs its own newer libwayland; loading the old
# bundled one makes WebKit abort with "Could not create default EGL display:
# EGL_BAD_PARAMETER" and the window stays blank.
#
# Deleting them outright is not enough: the bundled WebKit links
# libwayland-server.so.0, and Fedora's Mesa does not pull that in, so Fedora
# with an X11-only desktop (Xfce, MATE, i3) then failed with "error while
# loading shared libraries: libwayland-server.so.0". So each one moves to
# usr/lib/host-fallback/<soname>/, which no RUNPATH points at, and an AppRun
# hook adds that folder to LD_LIBRARY_PATH only when the host lacks the lib.
# A host without a libwayland has no Wayland-enabled Mesa to clash with.
# Proven by .github/workflows/cross-platform-build.yml (distros job).
set -euo pipefail
APP="$(readlink -f "$1")"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

(cd "$WORK" && "$APP" --appimage-extract >/dev/null)
ROOT="$WORK/squashfs-root"

for lib in "$ROOT"/usr/lib/libwayland-*; do
  dir="$ROOT/usr/lib/host-fallback/$(basename "$lib")"
  mkdir -p "$dir" && mv -v "$lib" "$dir/"
done

cat > "$ROOT/apprun-hooks/host-lib-fallback.sh" <<'EOF'
# Added by scripts/fix-appimage.sh: use a bundled library only if the host has none.
host_libs="$(/sbin/ldconfig -p 2>/dev/null || true)"
for d in "$APPDIR"/usr/lib/host-fallback/*/; do
  case "$host_libs" in
    *"$(basename "$d") (libc6,x86-64)"*) ;;
    *) export LD_LIBRARY_PATH="${d%/}${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" ;;
  esac
done
EOF
sed -i 's|^exec "$this_dir"/AppRun.wrapped|source "$this_dir"/apprun-hooks/host-lib-fallback.sh\n&|' "$ROOT/AppRun"
# linuxdeploy writes AppRun; if its format ever changes the hook must not go missing silently.
grep -q 'host-lib-fallback.sh' "$ROOT/AppRun" || { echo "AppRun format changed, hook not added"; exit 1; }

TOOL="$WORK/appimagetool"
curl -fsSL -o "$TOOL" \
  https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
chmod +x "$TOOL"
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" --no-appstream "$ROOT" "$APP"
echo "fixed: $APP"
