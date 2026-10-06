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
    # tauri finds the identity via `security find-identity -v`, which lists
    # only TRUSTED certificates; a self-signed one must be trusted for code
    # signing on the build machine. Users' Macs never need to trust it: TCC
    # matches the certificate's hash, not its trust.
    tmp=$(mktemp -d)
    echo "$APPLE_CERTIFICATE" | base64 --decode > "$tmp/cert.p12"
    openssl pkcs12 -in "$tmp/cert.p12" -nokeys -clcerts -passin env:APPLE_CERTIFICATE_PASSWORD -out "$tmp/cert.pem" 2>/dev/null \
      || openssl pkcs12 -legacy -in "$tmp/cert.p12" -nokeys -clcerts -passin env:APPLE_CERTIFICATE_PASSWORD -out "$tmp/cert.pem"
    sudo security add-trusted-cert -d -r trustRoot -p codeSign -k /Library/Keychains/System.keychain "$tmp/cert.pem"
    rm -rf "$tmp"
    echo "APPLE_SIGNING_IDENTITY=$IDENTITY" >> "$GITHUB_ENV"
    ;;
  verify)
    APP="$2"
    codesign --verify --deep --strict --verbose=2 "$APP"
    req=$(codesign -d -r- "$APP" 2>&1)
    echo "$req"
    # The stable parts TCC matches on. A cdhash-only requirement means ad-hoc.
    if ! grep -q 'identifier "app.silentvoice.desktop"' <<<"$req" \
       || ! grep -q 'certificate leaf = H"' <<<"$req"; then
      echo "::error::$APP is not signed with the Silent Voice certificate (requirement above)."
      exit 1
    fi
    echo "PASS: signed with a stable identity"
    ;;
  *) echo "usage: $0 setup | verify <app>"; exit 2 ;;
esac
