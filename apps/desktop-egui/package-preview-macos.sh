#!/usr/bin/env bash
# Side-by-side egui QA bundle, not a signed/stable application installer.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
[[ "$(uname -s)" == Darwin && "$(printenv CI || true)" == true ]] || { echo 'macOS CI only' >&2; exit 2; }
VERSION="$(tr -d '\r\n' < VERSION)"
ARCH="$(uname -m)"
OUT="$ROOT/dist/qa-egui"
APP="$OUT/WhereIsIt AI egui Preview.app"
BIN="$ROOT/target/release/atlas-desktop-egui"
[[ -x "$BIN" && ! -e "$APP" ]] || { echo 'Missing binary or existing QA bundle' >&2; exit 1; }
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/AtlasEgui"
chmod +x "$APP/Contents/MacOS/AtlasEgui"
cp apps/macos/Info.plist "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Set :CFBundleExecutable AtlasEgui' "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Set :CFBundleIdentifier ai.whereisit.egui.preview' "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Set :CFBundleName WhereIsIt AI egui Preview' "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist" >/dev/null
[[ "$(/usr/libexec/PlistBuddy -c 'Print :WhereIsItReleaseVersion' "$APP/Contents/Info.plist")" == "$VERSION" ]] || exit 1
lipo -archs "$APP/Contents/MacOS/AtlasEgui" | grep -qw "$ARCH" || { echo 'Wrong architecture' >&2; exit 1; }
cp LICENSE THIRD_PARTY_NOTICES.md "$APP/Contents/Resources/"
ditto licenses "$APP/Contents/Resources/licenses"
printf '%s\n' 'WhereIsIt AI egui QA preview: ad-hoc, not notarized; no physical GUI QA claimed.' 'No user CTF, SQLite or signing keys are bundled.' > "$APP/Contents/Resources/README-QA.txt"
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"
ZIP="$OUT/WhereIsItAI-egui-preview-$VERSION-macOS-$ARCH.zip"
[[ ! -e "$ZIP" ]] || { echo 'Existing QA archive' >&2; exit 1; }
ditto -c -k --sequesterRsrc --keepParent "$APP" "$ZIP"
(cd "$OUT" && shasum -a 256 "$(basename "$ZIP")" > "SHA256-$ARCH.txt")
printf 'scope=QA-preview-only\nversion=%s\ncommit=%s\narch=%s\nsigning=ad-hoc, not notarized\nuser_catalogs_included=false\n' "$VERSION" "$(git rev-parse HEAD)" "$ARCH" > "$OUT/provenance-$ARCH.txt"
echo 'PASS: side-by-side egui macOS QA app, ZIP and checksum'
