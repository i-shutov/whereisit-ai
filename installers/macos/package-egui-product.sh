#!/usr/bin/env bash
# Build side-by-side egui app, PKG and DMG; no private catalogs are bundled.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
[[ "$(uname -s)" == Darwin ]] || { echo 'Use a macOS builder.' >&2; exit 2; }
VERSION="$(tr -d '\r\n' < VERSION)"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+-rc\.[0-9]+$ ]] || { echo 'Egui product package is prerelease-only.' >&2; exit 2; }
APP="$ROOT/dist/macos-egui/WhereIsIt AI egui.app"
OUT="$ROOT/dist/macos-egui/installers"
BIN="$ROOT/target/release/atlas-desktop-egui"
CLI="$ROOT/target/release/atlas"
[[ -x "$BIN" && -x "$CLI" && ! -e "$APP" ]] || { echo 'Missing standard egui/CLI binary or existing app bundle.' >&2; exit 1; }
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$OUT"
cp "$BIN" "$APP/Contents/MacOS/WhereIsItAI"
cp "$CLI" "$APP/Contents/MacOS/whereisit-ai-cli"
chmod +x "$APP/Contents/MacOS/WhereIsItAI" "$APP/Contents/MacOS/whereisit-ai-cli"
cp apps/desktop-egui/Info.plist "$APP/Contents/Info.plist"
NUMERIC="${VERSION%%-*}"
RC="${VERSION##*.}"
BUILD_VERSION="$((10000 + 10#$RC))"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $NUMERIC" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $BUILD_VERSION" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :WhereIsItReleaseVersion $VERSION" "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist" >/dev/null
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$APP/Contents/Info.plist")" == WhereIsItAI ]]
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Contents/Info.plist")" == ai.whereisit.desktop.egui ]]
[[ "$("$APP/Contents/MacOS/whereisit-ai-cli" --version)" == "WhereIsIt AI $VERSION" ]]
ARCH="$(uname -m)"
lipo -archs "$APP/Contents/MacOS/WhereIsItAI" | grep -qw "$ARCH" || { echo "Wrong egui binary architecture: $ARCH" >&2; exit 1; }
cp LICENSE THIRD_PARTY_NOTICES.md "$APP/Contents/Resources/"
ditto licenses "$APP/Contents/Resources/licenses"
APP_IDENTITY="${MAC_APP_SIGN_IDENTITY:--}"
INSTALLER_IDENTITY="${MAC_INSTALLER_SIGN_IDENTITY:--}"
if [[ "$APP_IDENTITY" == '-' && "$INSTALLER_IDENTITY" != '-' ]] || [[ "$APP_IDENTITY" != '-' && "$INSTALLER_IDENTITY" == '-' ]]; then
  echo 'Provide both Developer ID Application and Installer identities, or neither.' >&2; exit 2
fi
if [[ "$APP_IDENTITY" == '-' ]]; then
  codesign --force --deep --sign - "$APP"
  SIGNING='ad-hoc app; unsigned pkg/dmg; not notarized'
else
  codesign --force --deep --options runtime --timestamp --sign "$APP_IDENTITY" "$APP"
  SIGNING='Developer ID signed, not notarized'
fi
codesign --verify --deep --strict "$APP"
if [[ "${NOTARIZE:-false}" == true ]]; then
  [[ "$APP_IDENTITY" != '-' && "$INSTALLER_IDENTITY" != '-' ]] || { echo 'Notarization requires Developer ID signatures.' >&2; exit 2; }
  : "${NOTARYTOOL_PROFILE:?Saved notarytool keychain profile required}"
  SIGNING='Developer ID signed and notarized'
fi
[[ "$APP/Contents/Info.plist" == *"WhereIsIt AI egui.app"* ]]
NUMERIC_VERSION="${VERSION%%-*}"
ARCH="$ARCH"
PKG="$OUT/WhereIsItAI-egui-$VERSION-macOS-$ARCH.pkg"
DMG="$OUT/WhereIsItAI-egui-$VERSION-macOS-$ARCH.dmg"
for target in "$PKG" "$DMG"; do [[ ! -e "$target" ]] || { echo "Refusing overwrite: $(basename "$target")" >&2; exit 1; }; done
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/pkg-root" "$TMP/dmg-root"
ditto "$APP" "$TMP/pkg-root/WhereIsIt AI egui.app"
ditto "$APP" "$TMP/dmg-root/WhereIsIt AI egui.app"
COMPONENTS="$ROOT/installers/macos/egui-components.plist"
plutil -lint "$COMPONENTS" >/dev/null
if [[ "$INSTALLER_IDENTITY" != '-' ]]; then
  pkgbuild --root "$TMP/pkg-root" --component-plist "$COMPONENTS" --install-location /Applications \
    --identifier ai.whereisit.desktop.egui --version "$NUMERIC_VERSION" --sign "$INSTALLER_IDENTITY" "$PKG"
else
  pkgbuild --root "$TMP/pkg-root" --component-plist "$COMPONENTS" --install-location /Applications \
    --identifier ai.whereisit.desktop.egui --version "$NUMERIC_VERSION" "$PKG"
fi
ln -s /Applications "$TMP/dmg-root/Applications"
hdiutil create -volname 'WhereIsIt AI egui' -srcfolder "$TMP/dmg-root" -format UDZO "$DMG"
if [[ "$APP_IDENTITY" != '-' ]]; then codesign --force --timestamp --sign "$APP_IDENTITY" "$DMG"; fi
if [[ "${NOTARIZE:-false}" == true ]]; then
  for package in "$PKG" "$DMG"; do
    xcrun notarytool submit "$package" --keychain-profile "$NOTARYTOOL_PROFILE" --wait
    xcrun stapler staple "$package"
    xcrun stapler validate "$package"
  done
fi
HASH="$OUT/SHA256-egui-$ARCH.txt"
(cd "$OUT" && shasum -a 256 "$(basename "$PKG")" "$(basename "$DMG")" > "$(basename "$HASH")")
APP_SHA="$(shasum -a 256 "$APP/Contents/MacOS/WhereIsItAI" | awk '{print $1}')"
CLI_SHA="$(shasum -a 256 "$APP/Contents/MacOS/whereisit-ai-cli" | awk '{print $1}')"
cat > "$OUT/provenance-egui-$ARCH.json" <<EOF
{
  "schema": "whereisit-egui-product-candidate-v1",
  "channel": "prerelease",
  "target_ui": "Rust/egui",
  "platform": "macos",
  "architecture": "$ARCH",
  "version": "$VERSION",
  "commit": "$(git rev-parse HEAD)",
  "run_id": "${GITHUB_RUN_ID:-local}",
  "signing": "$SIGNING",
  "stable_ready": false,
  "personal_catalogs_included": false,
  "app_sha256": "$APP_SHA",
  "cli_sha256": "$CLI_SHA"
}
EOF
python3 -m json.tool "$OUT/provenance-egui-$ARCH.json" >/dev/null
echo "Packaged egui product candidate for $ARCH ($SIGNING)"
