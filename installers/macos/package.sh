#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
[[ "$(uname -s)" == Darwin ]] || { echo 'Use a macOS builder.' >&2; exit 2; }
EXPECTED="$(tr -d '\r\n' < VERSION)"
VERSION="${VERSION:-$EXPECTED}"
[[ "$VERSION" == "$EXPECTED" ]] || { echo 'Package version must match VERSION' >&2; exit 2; }
APP="${APP:-$ROOT/dist/macos/WhereIsIt AI.app}"
OUT="$ROOT/dist/macos/installers"
mkdir -p "$OUT"
[[ -d "$APP" ]] || { echo 'Missing built app' >&2; exit 2; }
[[ "$(/usr/libexec/PlistBuddy -c 'Print :WhereIsItReleaseVersion' "$APP/Contents/Info.plist")" == "$VERSION" ]] || { echo 'Bundle version mismatch' >&2; exit 2; }
[[ "$("$APP/Contents/MacOS/whereisit-ai-cli" --version)" == "WhereIsIt AI $VERSION" ]] || { echo 'Bundled CLI version mismatch' >&2; exit 2; }
codesign --verify --deep --strict "$APP"
if [[ "${NOTARIZE:-false}" == true ]]; then
  : "${MAC_APP_SIGN_IDENTITY:?Developer ID Application identity required}"
  : "${MAC_INSTALLER_SIGN_IDENTITY:?Developer ID Installer identity required}"
  : "${NOTARYTOOL_PROFILE:?Saved notarytool keychain profile required}"
  [[ "$MAC_APP_SIGN_IDENTITY" != '-' ]] || { echo 'Cannot notarize an ad-hoc candidate' >&2; exit 2; }
fi
ARCH="$(uname -m)"
PKG="$OUT/WhereIsItAI-${VERSION}-macOS-${ARCH}.pkg"
DMG="$OUT/WhereIsItAI-${VERSION}-macOS-${ARCH}.dmg"
NUMERIC="${VERSION%%-*}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/pkg-root" "$TMP/dmg-root"
ditto "$APP" "$TMP/pkg-root/WhereIsIt AI.app"
# --component alone permits relocating to another discovered copy, including
# the build tree. Explicit component properties keep the install in /Applications.
COMPONENTS="$ROOT/installers/macos/components.plist"
plutil -lint "$COMPONENTS" >/dev/null
if [[ -n "${MAC_INSTALLER_SIGN_IDENTITY:-}" ]]; then
  pkgbuild --root "$TMP/pkg-root" --component-plist "$COMPONENTS" \
    --install-location /Applications --identifier ai.whereisit.desktop \
    --version "$NUMERIC" --sign "$MAC_INSTALLER_SIGN_IDENTITY" "$PKG"
else
  pkgbuild --root "$TMP/pkg-root" --component-plist "$COMPONENTS" \
    --install-location /Applications --identifier ai.whereisit.desktop \
    --version "$NUMERIC" "$PKG"
fi
ditto "$APP" "$TMP/dmg-root/WhereIsIt AI.app"
ln -s /Applications "$TMP/dmg-root/Applications"
hdiutil create -volname 'WhereIsIt AI' -srcfolder "$TMP/dmg-root" -format UDZO -ov "$DMG"
if [[ -n "${MAC_APP_SIGN_IDENTITY:-}" && "$MAC_APP_SIGN_IDENTITY" != '-' ]]; then
  codesign --force --timestamp --sign "$MAC_APP_SIGN_IDENTITY" "$DMG"
fi
if [[ "${NOTARIZE:-false}" == true ]]; then
  for package in "$PKG" "$DMG"; do
    xcrun notarytool submit "$package" --keychain-profile "$NOTARYTOOL_PROFILE" --wait
    xcrun stapler staple "$package"
    xcrun stapler validate "$package"
  done
fi
# Stapling mutates bytes: hash only the final downloadable packages.
(cd "$OUT" && shasum -a 256 "$(basename "$PKG")" "$(basename "$DMG")" > "SHA256-${ARCH}.txt")
printf 'Packaged %s and %s\n' "$PKG" "$DMG"
