#!/usr/bin/env bash
# Package integrity smoke for ephemeral macOS CI runners.
# Does NOT claim Gatekeeper acceptance, notarization or clean install.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
[[ "$(uname -s)" == Darwin ]] || { echo 'macOS only' >&2; exit 2; }
VERSION="$(printenv VERSION 2>/dev/null || echo 0.1.0)"
ARCH="$(uname -m)"
APP="$ROOT/dist/macos/WhereIsIt AI.app"
if [[ -n "$(printenv APP 2>/dev/null || true)" ]]; then APP="$(printenv APP)"; fi
PKG="$ROOT/dist/macos/installers/WhereIsItAI-$VERSION-macOS-$ARCH.pkg"
DMG="$ROOT/dist/macos/installers/WhereIsItAI-$VERSION-macOS-$ARCH.dmg"
[[ -d "$APP" && -f "$PKG" && -f "$DMG" ]] || {
  echo 'Missing built app/pkg/dmg' >&2; exit 2;
}
PLIST="$APP/Contents/Info.plist"
plutil -lint "$PLIST" >/dev/null
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleName' "$PLIST")" == "WhereIsIt AI" ]] || {
  echo 'Unexpected bundle name' >&2; exit 1;
}
ARCHS="$(lipo -archs "$APP/Contents/MacOS/AtlasCatalog")"
[[ " $ARCHS " == *" $ARCH "* ]] || {
  echo "Missing $ARCH binary slice" >&2; exit 1;
}
otool -L "$APP/Contents/MacOS/AtlasCatalog" | grep -q 'libatlas_ffi'
codesign --verify --deep --strict "$APP"
hdiutil verify "$DMG" >/dev/null
WORK="$(mktemp -d)"
MOUNT="$WORK/mount"
mkdir -p "$MOUNT"
MOUNTED=false
cleanup() {
  if [[ "$MOUNTED" == true ]]; then
    hdiutil detach "$MOUNT" -quiet || true
  fi
  rm -rf "$WORK"
}
trap cleanup EXIT
pkgutil --expand "$PKG" "$WORK/pkg"
[[ -f "$WORK/pkg/PackageInfo" ]] || { echo 'Missing PKG PackageInfo' >&2; exit 1; }
hdiutil attach "$DMG" -readonly -nobrowse -quiet -mountpoint "$MOUNT"
MOUNTED=true
[[ -d "$MOUNT/WhereIsIt AI.app" ]] || { echo 'DMG does not contain app' >&2; exit 1; }
[[ -L "$MOUNT/Applications" ]] || { echo 'DMG missing Applications link' >&2; exit 1; }
[[ "$(readlink "$MOUNT/Applications")" == /Applications ]] || {
  echo 'Unexpected Applications link target' >&2; exit 1;
}
echo "PASS: macOS $ARCH app linkage, package metadata and readable DMG"
echo 'Ad-hoc signing only; signing/notarization and clean Mac installation remain unverified.'
