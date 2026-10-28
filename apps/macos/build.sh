#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ "$(uname -s)" != Darwin ]]; then
  echo "Build this native SwiftUI application on macOS 13+ with Xcode Command Line Tools." >&2
  exit 1
fi
export MACOSX_DEPLOYMENT_TARGET=13.0
cargo build --release -p atlas-ffi
root="$PWD"
cd apps/macos
swift build -c release -Xlinker -L -Xlinker "$root/target/release" \
  -Xlinker -rpath -Xlinker '@executable_path/../Frameworks'
binary_dir="$(swift build -c release --show-bin-path)"
app="$root/dist/macos/WhereIsIt AI.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Frameworks"
cp "$binary_dir/AtlasCatalog" "$app/Contents/MacOS/AtlasCatalog"
cp "$root/target/release/libatlas_ffi.dylib" "$app/Contents/Frameworks/libatlas_ffi.dylib"
cp Info.plist "$app/Contents/Info.plist"
install_name_tool -id '@rpath/libatlas_ffi.dylib' "$app/Contents/Frameworks/libatlas_ffi.dylib"
while IFS= read -r dependency; do
  install_name_tool -change "$dependency" '@rpath/libatlas_ffi.dylib' "$app/Contents/MacOS/AtlasCatalog"
done < <(otool -L "$app/Contents/MacOS/AtlasCatalog" | awk '/libatlas_ffi/{print $1}')
codesign --force --sign - "$app/Contents/Frameworks/libatlas_ffi.dylib"
codesign --force --sign - "$app"
echo "Built: $app"
echo "This local build is ad-hoc signed. Distribution requires Developer ID signing and notarization."
