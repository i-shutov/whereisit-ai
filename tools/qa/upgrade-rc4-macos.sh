#!/usr/bin/env bash
# Standard egui product upgrade and reinstall on a disposable hosted runner.
set -euo pipefail
[[ "$(uname -s)" == Darwin && "${GITHUB_ACTIONS:-}" == true &&
   "${RUNNER_ENVIRONMENT:-}" == github-hosted && -n "${RUNNER_TEMP:-}" && $# == 1 ]] ||
  { echo 'Hosted GitHub Actions macOS runner and RC3 root required.' >&2; exit 2; }
SOURCE="$(cd "$1" && pwd)"
TARGET="$(cd "$(dirname "$0")/../.." && pwd)"
SOURCE_SHA="$(git -C "$SOURCE" rev-parse HEAD)"
TARGET_SHA="$(git -C "$TARGET" rev-parse HEAD)"
[[ "$SOURCE_SHA" == 0708f0444f035fc44946c224afb18e847efca1de ]]
[[ "$TARGET_SHA" =~ ^[0-9a-f]{40}$ && "$TARGET_SHA" == "${RC4_SHA:?RC4_SHA required}" ]]
ARCH="$(uname -m)"
[[ "$ARCH" == arm64 || "$ARCH" == x86_64 ]]
SOURCE_PKG="$SOURCE/dist/macos-egui/installers/WhereIsItAI-egui-1.0.0-rc.3-macOS-$ARCH.pkg"
TARGET_PKG="$TARGET/dist/macos-egui/installers/WhereIsItAI-egui-1.0.0-rc.4-macOS-$ARCH.pkg"
APP='/Applications/WhereIsIt AI egui.app'
RECEIPT=ai.whereisit.desktop.egui
CLI="$APP/Contents/MacOS/whereisit-ai-cli"
if [[ -e "$APP" || -L "$APP" ]] || pkgutil --pkg-info "$RECEIPT" >/dev/null 2>&1; then
  echo 'Refusing to replace an existing egui installation.' >&2
  exit 2
fi
WORK="$(mktemp -d "$RUNNER_TEMP/whereisit-egui-upgrade.XXXXXX")"
DB="$WORK/external.sqlite"
FIXTURE="$WORK/fixture"
SNAPSHOT="$WORK/baseline.json"
PLACEHOLDER="$FIXTURE/external-user-ctf-placeholder.CTF"
CREATED=false
cleanup() {
  if [[ "$CREATED" == true ]]; then
    sudo rm -rf -- "$APP"
    sudo pkgutil --forget "$RECEIPT" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

sha() {
  shasum -a 256 "$1" | awk '{print $1}'
}
install_product() {
  local package="$1" build_root="$2" version="$3" build="$4" name
  local staged="$build_root/dist/macos-egui/WhereIsIt AI egui.app"
  CREATED=true
  sudo installer -pkg "$package" -target /
  [[ "$("$CLI" --version)" == "WhereIsIt AI $version" ]]
  [[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Contents/Info.plist")" == "$RECEIPT" ]]
  [[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$APP/Contents/Info.plist")" == "$build" ]]
  [[ "$(/usr/libexec/PlistBuddy -c 'Print :WhereIsItReleaseVersion' "$APP/Contents/Info.plist")" == "$version" ]]
  for name in WhereIsItAI whereisit-ai-cli; do
    [[ "$(sha "$APP/Contents/MacOS/$name")" == "$(sha "$staged/Contents/MacOS/$name")" ]]
  done
  codesign --verify --deep --strict "$APP"
  pkgutil --pkg-info "$RECEIPT" >/dev/null
}
remove_product() {
  sudo rm -rf -- "$APP"
  sudo pkgutil --forget "$RECEIPT" >/dev/null
  [[ ! -e "$APP" && ! -L "$APP" ]]
  CREATED=false
}
run_probe() {
  python3 "$TARGET/tools/qa/upgrade_catalog_probe.py" "$1" --cli "$CLI" --db "$DB" \
    --fixture-root "$FIXTURE" --snapshot "$SNAPSHOT" \
    --source-release 1.0.0-rc.3 --target-release 1.0.0-rc.4
}
verify_external() {
  [[ "$(sha "$DB")" == "$1" && "$(sha "$PLACEHOLDER")" == "$CTF_SHA" ]] ||
    { echo 'Installer lifecycle changed generated external data.' >&2; exit 1; }
}

SOURCE_PKG_SHA="$(sha "$SOURCE_PKG")"
TARGET_PKG_SHA="$(sha "$TARGET_PKG")"
install_product "$SOURCE_PKG" "$SOURCE" 1.0.0-rc.3 1003
run_probe prepare
BEFORE_UPGRADE="$(sha "$DB")"
CTF_SHA="$(sha "$PLACEHOLDER")"
install_product "$TARGET_PKG" "$TARGET" 1.0.0-rc.4 10004
verify_external "$BEFORE_UPGRADE"
run_probe verify
BEFORE_ROLLBACK="$(sha "$DB")"
remove_product
verify_external "$BEFORE_ROLLBACK"
install_product "$SOURCE_PKG" "$SOURCE" 1.0.0-rc.3 1003
verify_external "$BEFORE_ROLLBACK"
"$CLI" validate "$DB" >/dev/null
remove_product
verify_external "$BEFORE_ROLLBACK"

OUT="$TARGET/dist/qa-egui-upgrade"
mkdir -p "$OUT"
python3 - "$OUT/UPGRADE.json" "$ARCH" "$SOURCE_SHA" "$TARGET_SHA" "$SOURCE_PKG_SHA" "$TARGET_PKG_SHA" <<'PY'
import json
import sys
from pathlib import Path

output, arch, source_commit, target_commit, source_package, target_package = sys.argv[1:]
data = {
    "format": "whereisit-egui-rc3-to-rc4-upgrade-v1",
    "platform": f"macos-{arch}",
    "source_version": "1.0.0-rc.3",
    "target_version": "1.0.0-rc.4",
    "source_commit": source_commit,
    "target_commit": target_commit,
    "source_package_sha256": source_package,
    "target_package_sha256": target_package,
    "external_data_preserved": True,
    "catalog_search_and_category_parity": True,
    "rollback_reinstall_and_validation": True,
    "synthetic_only": True,
    "gui_observed": False,
    "status": "pass",
}
Path(output).write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
PY
