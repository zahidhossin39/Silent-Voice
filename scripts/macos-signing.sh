#!/usr/bin/env bash
# macOS code signing with the project's own (free, self-signed) certificate.
#
# Why: macOS ties an Accessibility grant to the app's code signature. Unsigned
# (ad-hoc) builds get a new identity every build, so after each update the
# System Settings toggle still showed ON but the app was no longer trusted.
# Signing every build with the SAME certificate keeps the identity stable:
#   identifier "app.silentvoice.desktop" and certificate leaf = H"<cert hash>"
# It does not satisfy Gatekeeper (that needs Apple's paid Developer ID).
#
# Usage (CI, macOS only):
#   bash scripts/macos-signing.sh setup          # before `tauri build`
#   bash scripts/macos-signing.sh verify <.app>  # after it
# setup needs the APPLE_CERTIFICATE (base64 .p12) and APPLE_CERTIFICATE_PASSWORD
# secrets in the environment; tauri build imports and uses them.
set -euo pipefail
IDENTITY="Silent Voice Self-Signed"

case "${1:-}" in
  setup)
    if [ -z "${APPLE_CERTIFICATE:-}" ] || [ -z "${APPLE_CERTIFICATE_PASSWORD:-}" ]; then
      echo "::error::APPLE_CERTIFICATE / APPLE_CERTIFICATE_PASSWORD secrets are missing. An unsigned macOS build loses its Accessibility permission on every update."
      exit 1
    fi
    # Import it ourselves. Given APPLE_CERTIFICATE, tauri imports it but only
    # accepts Apple-issued names ("Developer ID Application: ..."), so a
    # self-signed one fails with "failed to resolve signing identity". Given
    # only APPLE_SIGNING_IDENTITY, tauri hands the name straight to codesign.
    # So the build steps must NOT see APPLE_CERTIFICATE.
    tmp=$(mktemp -d)
    kc="$HOME/Library/Keychains/silent-voice-signing.keychain-db"
    kcpw=$(openssl rand -hex 16)
    echo "$APPLE_CERTIFICATE" | base64 --decode > "$tmp/cert.p12"
    security create-keychain -p "$kcpw" "$kc"
    security set-keychain-settings -t 3600 -u "$kc"
    security unlock-keychain -p "$kcpw" "$kc"
    security import "$tmp/cert.p12" -k "$kc" -P "$APPLE_CERTIFICATE_PASSWORD" -T /usr/bin/codesign
    security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$kcpw" "$kc" >/dev/null
    security list-keychains -d user -s "$kc" $(security list-keychains -d user | tr -d '"')
    # codesign also wants the certificate trusted for code signing on the
    # build machine. Users' Macs never need to trust it: TCC matches the
    # certificate's hash, not its trust.
    security find-certificate -c "$IDENTITY" -p "$kc" > "$tmp/cert.pem"
    sudo security add-trusted-cert -d -r trustRoot -p codeSign -k /Library/Keychains/System.keychain "$tmp/cert.pem"
    rm -rf "$tmp"
    security find-identity -p codesigning "$kc"
    echo "APPLE_SIGNING_IDENTITY=$IDENTITY" >> "$GITHUB_ENV"
    ;;
  verify)
    APP="$2"
    codesign --verify --deep --strict --verbose=2 "$APP"
    req=$(codesign -d -r- "$APP" 2>&1)
    echo "$req"
    # The stable parts TCC matches on. A cdhash-only requirement means ad-hoc.
    # A self-signed cert is its own root, so codesign may pin it as "root".
    dr=$(grep -o 'designated => .*' <<<"$req" || true)
    if ! grep -q 'identifier "app.silentvoice.desktop"' <<<"$dr" \
       || ! grep -qE 'certificate (leaf|root) = H"' <<<"$dr"; then
      echo "::error::$APP is not signed with the Silent Voice certificate. Requirement: ${dr:-$req}"
      exit 1
    fi
    echo "::notice title=macOS signature::$dr"
    echo "PASS: signed with a stable identity"
    ;;
  *) echo "usage: $0 setup | verify <app>"; exit 2 ;;
esac
