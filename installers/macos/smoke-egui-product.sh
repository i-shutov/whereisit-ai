#!/usr/bin/env bash
# CI-only package/lifecycle smoke with generated SQLite data; not GUI or notarization acceptance.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
[[ "$(uname -s)" == Darwin && "${CI:-}" == true ]] || { echo 'macOS CI only' >&2; exit 2; }
VERSION="$(tr -d '\r\n' < VERSION)"
ARCH="$(uname -m)"
APP="$ROOT/dist/macos-egui/WhereIsIt AI egui.app"
PKG="$ROOT/dist/macos-egui/installers/WhereIsItAI-egui-$VERSION-macOS-$ARCH.pkg"
DMG="$ROOT/dist/macos-egui/installers/WhereIsItAI-egui-$VERSION-macOS-$ARCH.dmg"
[[ -d "$APP" && -f "$PKG" && -f "$DMG" ]] || { echo 'Missing egui app/pkg/dmg.' >&2; exit 2; }
PLIST="$APP/Contents/Info.plist"
plutil -lint "$PLIST" >/dev/null
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleName' "$PLIST")" == 'WhereIsIt AI' ]]
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$PLIST")" == 'WhereIsItAI' ]]
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$PLIST")" == 'ai.whereisit.desktop.egui' ]]
[[ "$(/usr/libexec/PlistBuddy -c 'Print :WhereIsItReleaseVersion' "$PLIST")" == "$VERSION" ]]
RC="${VERSION##*.}"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$PLIST")" == "$((10000 + 10#$RC))" ]]
lipo -archs "$APP/Contents/MacOS/WhereIsItAI" | grep -qw "$ARCH" || { echo 'Wrong egui architecture.' >&2; exit 1; }
codesign --verify --deep --strict "$APP"
for file in LICENSE THIRD_PARTY_NOTICES.md "licenses/SQLite-$(python3 -c 'import runpy; print(runpy.run_path("tools/dev/refresh_sqlite_notices.py")["SQLITE_VERSION"])')/COPYRIGHT.txt"; do
  [[ -f "$APP/Contents/Resources/$file" ]] || { echo "Missing app notice: $file" >&2; exit 1; }
done
[[ "$("$APP/Contents/MacOS/whereisit-ai-cli" --version)" == "WhereIsIt AI $VERSION" ]]
(cd "$(dirname "$DMG")" && shasum -a 256 -c "SHA256-egui-$ARCH.txt" >/dev/null)
bash "$ROOT/installers/macos/verify-dmg.sh" "$DMG"
WORK="$(mktemp -d)"; MOUNT="$WORK/mount"; mkdir -p "$MOUNT"
MOUNTED=false; INSTALLED='/Applications/WhereIsIt AI egui.app'; CREATED_INSTALL=false
cleanup() {
  if [[ "$MOUNTED" == true ]]; then hdiutil detach "$MOUNT" -quiet || true; fi
  if [[ "$CREATED_INSTALL" == true ]]; then sudo rm -rf -- "$INSTALLED"; sudo pkgutil --forget ai.whereisit.desktop.egui >/dev/null || true; fi
  rm -rf -- "$WORK"
}
trap cleanup EXIT
pkgutil --expand "$PKG" "$WORK/pkg"
python3 - "$WORK/pkg/PackageInfo" <<'PY'
import sys
import xml.etree.ElementTree as ET
info = ET.parse(sys.argv[1]).getroot()
assert info.attrib.get('install-location') == '/Applications'
assert info.attrib.get('identifier') == 'ai.whereisit.desktop.egui'
for element in info.iter():
    if element.tag.rsplit('}', 1)[-1] == 'relocate':
        assert len(element) == 0
print('PASS: egui package targets /Applications with fixed bundle identity')
PY
hdiutil attach "$DMG" -readonly -nobrowse -quiet -mountpoint "$MOUNT"; MOUNTED=true
[[ -d "$MOUNT/WhereIsIt AI egui.app" && -L "$MOUNT/Applications" ]]
[[ "$(readlink "$MOUNT/Applications")" == /Applications ]]
[[ ! -e "$INSTALLED" && ! -L "$INSTALLED" ]] || { echo 'Existing egui app; refusing smoke install.' >&2; exit 1; }
if pkgutil --pkg-info ai.whereisit.desktop.egui >/dev/null 2>&1; then echo 'Existing egui package receipt; refusing smoke install.' >&2; exit 1; fi
WORK_CATALOG="$WORK/catalog.sqlite"; WORK_COMPACT="$WORK/compact.sqlite"
CREATED_INSTALL=true
sudo installer -pkg "$PKG" -target /
CLI="$INSTALLED/Contents/MacOS/whereisit-ai-cli"
[[ -x "$INSTALLED/Contents/MacOS/WhereIsItAI" && -x "$CLI" ]]
[[ "$("$CLI" --version)" == "WhereIsIt AI $VERSION" ]]
python3 -c 'import json,sys; print(json.dumps({"op":"init","db":sys.argv[1]}))' "$WORK_CATALOG" | "$CLI" serve > "$WORK/init.json"
python3 -c 'import json,sys; assert json.load(open(sys.argv[1]))["ok"] is True' "$WORK/init.json"
"$CLI" validate "$WORK_CATALOG" >/dev/null
BEFORE="$(shasum -a 256 "$WORK_CATALOG" | awk '{print $1}')"
"$CLI" compact "$WORK_CATALOG" "$WORK_COMPACT" >/dev/null
[[ "$(shasum -a 256 "$WORK_CATALOG" | awk '{print $1}')" == "$BEFORE" ]]
sudo rm -rf -- "$INSTALLED"; sudo pkgutil --forget ai.whereisit.desktop.egui >/dev/null; CREATED_INSTALL=false
[[ -f "$WORK_CATALOG" && -f "$WORK_COMPACT" ]]
echo "PASS: egui $ARCH app/PKG/DMG, generated catalog survives install/uninstall; GUI usability and signing remain separate gates"
