#!/usr/bin/env bash
# Build distributable .pkg and .dmg from an already-built .app (macOS only).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
if [[ "$(uname -s)" != Darwin ]]; then
  echo 'Use a macOS builder with pkgbuild and hdiutil.' >&2
  exit 2
fi
VERSION="${VERSION:-0.1.0}"
APP="${APP:-$ROOT/dist/macos/WhereIsIt AI.app}"
OUT="$ROOT/dist/macos/installers"
mkdir -p "$OUT"
[[ -d "$APP" ]] || { echo "Missing application: $APP" >&2; exit 2; }
ARCH="$(uname -m)"
PKG="$OUT/WhereIsItAI-${VERSION}-macOS-${ARCH}.pkg"
DMG="$OUT/WhereIsItAI-${VERSION}-macOS-${ARCH}.dmg"
# The .app may be ad-hoc signed for CI smoke tests. Distribution requires proper signing.
if [[ -n "${MAC_INSTALLER_SIGN_IDENTITY:-}" ]]; then
  pkgbuild --component "$APP" --install-location /Applications \
    --identifier ai.whereisit.desktop --version "$VERSION" \
    --sign "$MAC_INSTALLER_SIGN_IDENTITY" "$PKG"
else
  pkgbuild --component "$APP" --install-location /Applications \
    --identifier ai.whereisit.desktop --version "$VERSION" "$PKG"
fi
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cp -R "$APP" "$TMP/WhereIsIt AI.app"
ln -s /Applications "$TMP/Applications"
hdiutil create -volname 'WhereIsIt AI' -srcfolder "$TMP" \
  -format UDZO -ov "$DMG"
shasum -a 256 "$PKG" "$DMG" > "$OUT/SHA256-${ARCH}.txt"
if [[ "${NOTARIZE:-false}" == true ]]; then
  : "${NOTARYTOOL_PROFILE:?Set a saved notarytool keychain profile for signed notarization}"
  xcrun notarytool submit "$PKG" --keychain-profile "$NOTARYTOOL_PROFILE" --wait
  xcrun stapler staple "$PKG"
fi
printf 'Packaged %s and %s\n' "$PKG" "$DMG"
