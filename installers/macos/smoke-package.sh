#!/usr/bin/env bash
# Isolated CI runner only: validates packages and installs/removes this app.
# This does not claim GUI usability or Gatekeeper/notarization acceptance.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
[[ "$(uname -s)" == Darwin ]] || { echo 'macOS only' >&2; exit 2; }
VERSION="$(tr -d '\r\n' < VERSION)"
ARCH="$(uname -m)"
APP="$ROOT/dist/macos/WhereIsIt AI.app"
PKG="$ROOT/dist/macos/installers/WhereIsItAI-$VERSION-macOS-$ARCH.pkg"
DMG="$ROOT/dist/macos/installers/WhereIsItAI-$VERSION-macOS-$ARCH.dmg"
[[ -d "$APP" && -f "$PKG" && -f "$DMG" ]] || { echo 'Missing app/pkg/dmg' >&2; exit 2; }
PLIST="$APP/Contents/Info.plist"
plutil -lint "$PLIST" >/dev/null
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleName' "$PLIST")" == 'WhereIsIt AI' ]] || exit 1
[[ "$(/usr/libexec/PlistBuddy -c 'Print :WhereIsItReleaseVersion' "$PLIST")" == "$VERSION" ]] || exit 1
ARCHS="$(lipo -archs "$APP/Contents/MacOS/AtlasCatalog")"
[[ " $ARCHS " == *" $ARCH "* ]] || { echo 'Wrong architecture' >&2; exit 1; }
otool -L "$APP/Contents/MacOS/AtlasCatalog" | grep -q 'libatlas_ffi'
codesign --verify --deep --strict "$APP"
(cd "$(dirname "$DMG")" && shasum -a 256 -c "SHA256-$ARCH.txt" >/dev/null)
bash "$ROOT/installers/macos/verify-dmg.sh" "$DMG"
WORK="$(mktemp -d)"
MOUNT="$WORK/mount"
mkdir -p "$MOUNT"
MOUNTED=false
INSTALLED='/Applications/WhereIsIt AI.app'
CREATED_INSTALL=false
cleanup() {
  if [[ "$MOUNTED" == true ]]; then hdiutil detach "$MOUNT" -quiet || true; fi
  if [[ "$CREATED_INSTALL" == true ]]; then
    sudo rm -rf -- "$INSTALLED"
    sudo pkgutil --forget ai.whereisit.desktop >/dev/null || true
  fi
  rm -rf -- "$WORK"
}
trap cleanup EXIT
pkgutil --expand "$PKG" "$WORK/pkg"
[[ -f "$WORK/pkg/PackageInfo" ]] || { echo 'Missing PackageInfo' >&2; exit 1; }
python3 - "$WORK/pkg/PackageInfo" <<'PY'
import sys
import xml.etree.ElementTree as ET
info = ET.parse(sys.argv[1]).getroot()
assert info.attrib.get('install-location') == '/Applications', 'Unexpected package destination'
assert info.attrib.get('identifier') == 'ai.whereisit.desktop', 'Unexpected package identifier'
for element in info.iter():
    if element.tag.rsplit('}', 1)[-1] == 'relocate':
        assert len(element) == 0, 'Relocatable bundle found in installer'
print('PASS: PKG destination fixed to /Applications; no relocation rules')
PY
hdiutil attach "$DMG" -readonly -nobrowse -quiet -mountpoint "$MOUNT"
MOUNTED=true
[[ -d "$MOUNT/WhereIsIt AI.app" && -L "$MOUNT/Applications" ]] || exit 1
[[ "$(readlink "$MOUNT/Applications")" == /Applications ]] || exit 1
# Never remove or replace an existing application/receipt on any machine.
[[ ! -e "$INSTALLED" && ! -L "$INSTALLED" ]] || { echo 'Existing app; refusing smoke install' >&2; exit 1; }
if pkgutil --pkg-info ai.whereisit.desktop >/dev/null 2>&1; then echo 'Existing receipt; refusing smoke install' >&2; exit 1; fi
[[ "${CI:-}" == true ]] || { echo 'Installation smoke is restricted to CI runners' >&2; exit 1; }
CREATED_INSTALL=true
sudo installer -pkg "$PKG" -target /
CLI="$INSTALLED/Contents/MacOS/whereisit-ai-cli"
if [[ ! -x "$CLI" ]]; then
  echo 'Installer returned without the expected executable in /Applications' >&2
  pkgutil --files ai.whereisit.desktop || true
  exit 1
fi
for notice in LICENSE THIRD_PARTY_NOTICES.md licenses/SQLite-3.51.3/COPYRIGHT.txt; do
  [[ -f "$INSTALLED/Contents/Resources/$notice" ]] || { echo "Missing installed notice: $notice" >&2; exit 1; }
done
[[ "$("$CLI" --version)" == "WhereIsIt AI $VERSION" ]] || exit 1
SQLITE_ACTUAL="$("$CLI" --sqlite-version)"
[[ "$SQLITE_ACTUAL" == '3.51.3' ]] || { echo "Installed CLI has incorrect SQLite: $SQLITE_ACTUAL" >&2; exit 1; }
echo "PASS: installed $ARCH CLI embeds SQLite $SQLITE_ACTUAL"
python3 -c 'import json,sys; print(json.dumps({"op":"init","db":sys.argv[1]}))' "$WORK/catalog.sqlite" | "$CLI" serve > "$WORK/init.json"
python3 -c 'import json,sys; assert json.load(open(sys.argv[1]))["ok"] is True' "$WORK/init.json"
"$CLI" validate "$WORK/catalog.sqlite" > "$WORK/validation.json"
BEFORE="$(shasum -a 256 "$WORK/catalog.sqlite" | awk '{print $1}')"
"$CLI" compact "$WORK/catalog.sqlite" "$WORK/compact.sqlite" > "$WORK/compact.json"
AFTER="$(shasum -a 256 "$WORK/catalog.sqlite" | awk '{print $1}')"
[[ "$BEFORE" == "$AFTER" ]] || { echo 'Source bytes changed' >&2; exit 1; }
sudo rm -rf -- "$INSTALLED"
sudo pkgutil --forget ai.whereisit.desktop >/dev/null
CREATED_INSTALL=false
[[ -f "$WORK/catalog.sqlite" && -f "$WORK/compact.sqlite" ]] || { echo 'User catalog lost during removal' >&2; exit 1; }
echo "PASS: $ARCH app, PKG install, license resources, bundled CLI, read-only validation, compact copy, removal preserves data, DMG integrity."
echo 'GUI usability, Gatekeeper and Developer ID notarization are separate gates.'
