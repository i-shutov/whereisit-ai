#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ "$(uname -s)" != Darwin ]]; then
  echo 'Build on macOS 13+ with Xcode Command Line Tools.' >&2
  exit 1
fi
export MACOSX_DEPLOYMENT_TARGET=13.0
version="$(tr -d '\r\n' < VERSION)"
cargo build --release --locked -p atlas-ffi -p atlas-cli
[[ "$(target/release/atlas --version)" == "WhereIsIt AI $version" ]] || { echo 'CLI version mismatch' >&2; exit 1; }
root="$PWD"
cd apps/macos
swift build -c release -Xlinker -L -Xlinker "$root/target/release" \
  -Xlinker -rpath -Xlinker '@executable_path/../Frameworks'
binary_dir="$(swift build -c release --show-bin-path)"
app="$root/dist/macos/WhereIsIt AI.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Frameworks"
cp "$binary_dir/AtlasCatalog" "$app/Contents/MacOS/AtlasCatalog"
cp "$root/target/release/atlas" "$app/Contents/MacOS/whereisit-ai-cli"
cp "$root/target/release/libatlas_ffi.dylib" "$app/Contents/Frameworks/libatlas_ffi.dylib"
cp Info.plist "$app/Contents/Info.plist"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :WhereIsItReleaseVersion' "$app/Contents/Info.plist")" == "$version" ]] || { echo 'Bundle version mismatch' >&2; exit 1; }
install_name_tool -id '@rpath/libatlas_ffi.dylib' "$app/Contents/Frameworks/libatlas_ffi.dylib"
while IFS= read -r dependency; do
  install_name_tool -change "$dependency" '@rpath/libatlas_ffi.dylib' "$app/Contents/MacOS/AtlasCatalog"
done < <(otool -L "$app/Contents/MacOS/AtlasCatalog" | awk '/libatlas_ffi/{print $1}')
identity="${MAC_APP_SIGN_IDENTITY:--}"
options=()
if [[ "$identity" != '-' ]]; then options=(--options runtime --timestamp); fi
codesign --force --sign "$identity" "${options[@]}" "$app/Contents/Frameworks/libatlas_ffi.dylib"
codesign --force --sign "$identity" "${options[@]}" "$app/Contents/MacOS/whereisit-ai-cli"
codesign --force --sign "$identity" "${options[@]}" "$app"
codesign --verify --deep --strict "$app"
printf 'Built: %s\nRelease: %s\n' "$app" "$version"
if [[ "$identity" == '-' ]]; then echo 'AD-HOC SIGNED TEST BUILD; not Developer ID signed or notarized.'; fi
