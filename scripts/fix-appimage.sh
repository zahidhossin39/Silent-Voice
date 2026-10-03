#!/usr/bin/env bash
# Remove the bundled libwayland-* from an AppImage, in place.
# Usage: bash scripts/fix-appimage.sh path/to/app.AppImage
#
# Why: the AppImage is built on Ubuntu 22.04 and bundles that release's
# libwayland, but it always uses the HOST's Mesa EGL driver. On newer distros
# (Arch, Fedora) that Mesa needs its own newer libwayland; loading the old
# bundled one makes WebKit abort with "Could not create default EGL display:
# EGL_BAD_PARAMETER" and the window stays blank. Every Linux desktop ships
# libwayland itself (GTK and Mesa depend on it), so the system copy is always
# there. Proven by .github/workflows/linux-distro-smoke.yml.
set -euo pipefail
APP="$(readlink -f "$1")"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

(cd "$WORK" && "$APP" --appimage-extract >/dev/null)
rm -fv "$WORK"/squashfs-root/usr/lib/libwayland-*

TOOL="$WORK/appimagetool"
curl -fsSL -o "$TOOL" \
  https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
chmod +x "$TOOL"
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" --no-appstream "$WORK/squashfs-root" "$APP"
echo "fixed: $APP"
