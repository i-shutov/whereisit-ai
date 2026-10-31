#!/usr/bin/env bash
# Real PKG-on-PKG upgrade; strictly isolated CI only, generated fixtures only.
set -euo pipefail
[[ "$(uname -s)" == Darwin && "$(printenv CI || true)" == true && -n "$(printenv RUNNER_TEMP || true)" ]] ||
  { echo "Upgrade QA must run on an ephemeral macOS CI runner" >&2; exit 2; }
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
VERSION="$(tr -d '\r\n' < VERSION)"
[[ "$VERSION" == '1.0.0-rc.3' ]] || { echo "Refusing wrong version" >&2; exit 2; }
ARCH="$(uname -m)"
case "$ARCH" in
  arm64) OLD_SHA='fd62dc16e8e08c6b125b3dc97f8b1b5bdd252b11ae0ce997722a0ce1397616df' ;;
  x86_64) OLD_SHA='6a9b20a2523c9426150431a2bb2cfccf7110ec0cdd7f46d6951b28058a1ceb69' ;;
  *) echo "Unknown macOS architecture" >&2; exit 2 ;;
esac
OLD_NAME="WhereIsItAI-1.0.0-rc.2-macOS-$ARCH.pkg"
NEW_PKG="$ROOT/dist/macos/installers/WhereIsItAI-$VERSION-macOS-$ARCH.pkg"
[[ -f "$NEW_PKG" ]] || { echo "New package not built" >&2; exit 2; }
APP="/Applications/WhereIsIt AI.app"
RECEIPT="ai.whereisit.desktop"
# Existing user app and receipt are absolute blockers. Do not run against a
# development workstation or uninstall someone else's application.
[[ ! -e "$APP" && ! -L "$APP" ]] || { echo "App already installed; refusing test" >&2; exit 2; }
if pkgutil --pkg-info "$RECEIPT" >/dev/null 2>&1; then
  echo "Package receipt already installed; refusing upgrade test" >&2
  exit 2
fi
WORK="$(mktemp -d "$RUNNER_TEMP/whereisit-upgrade.XXXXXXXX")"
CREATED=false
cleanup() {
  if [[ "$CREATED" == true ]]; then
    sudo rm -rf -- "$APP"
    sudo pkgutil --forget "$RECEIPT" >/dev/null 2>&1 || true
  fi
  rm -rf -- "$WORK"
}
trap cleanup EXIT
OLD="$WORK/$OLD_NAME"
curl --fail --location --silent --show-error --retry 3 --retry-delay 2 \
  "https://github.com/i-shutov/whereisit-ai/releases/download/v1.0.0-rc.2/$OLD_NAME" \
  --output "$OLD"
ACTUAL_OLD="$(shasum -a 256 "$OLD" | awk '{print $1}')"
[[ "$ACTUAL_OLD" == "$OLD_SHA" ]] ||
  { echo "SHA-256 pin mismatch for old package" >&2; exit 1; }

# Verify current candidate metadata before mutating any CI system directory.
plutil -lint "$ROOT/dist/macos/WhereIsIt AI.app/Contents/Info.plist" >/dev/null
codesign --verify --deep --strict "$ROOT/dist/macos/WhereIsIt AI.app"
[[ "$("$ROOT/dist/macos/WhereIsIt AI.app/Contents/MacOS/whereisit-ai-cli" --version)" == "WhereIsIt AI $VERSION" ]]
NEW_SHA="$(shasum -a 256 "$NEW_PKG" | awk '{print $1}')"

# Only the clean CI application path is installed/overwritten in this test.
sudo installer -pkg "$OLD" -target /
CREATED=true
CLI="$APP/Contents/MacOS/whereisit-ai-cli"
[[ -x "$CLI" && "$("$CLI" --version)" == "WhereIsIt AI 1.0.0-rc.2" ]] ||
  { echo "Expected RC2 installed CLI" >&2; exit 1; }
OLD_BUILD="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$APP/Contents/Info.plist")"
[[ "$OLD_BUILD" == 10002 ]] || { echo "RC2 bundle build mismatch" >&2; exit 1; }

DB="$WORK/user-catalog.sqlite"
FIXTURE="$WORK/synthetic-fixture"
BASELINE="$WORK/baseline.json"
python3 tools/qa/upgrade_catalog_probe.py prepare \
  --cli "$CLI" --db "$DB" --fixture-root "$FIXTURE" --snapshot "$BASELINE"
DB_SHA_BEFORE="$(shasum -a 256 "$DB" | awk '{print $1}')"

# The actual in-place upgrade, same package identifier and /Applications path.
sudo installer -pkg "$NEW_PKG" -target /
[[ -x "$CLI" && "$("$CLI" --version)" == "WhereIsIt AI $VERSION" ]] ||
  { echo "RC3 installed CLI did not replace RC2" >&2; exit 1; }
[[ "$("$CLI" --sqlite-version)" == 3.51.3 ]] || { echo "Embedded SQLite mismatch" >&2; exit 1; }
NEW_BUILD="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$APP/Contents/Info.plist")"
[[ "$NEW_BUILD" == 10003 ]] || { echo "RC3 bundle build not monotonic" >&2; exit 1; }
[[ "$(/usr/libexec/PlistBuddy -c 'Print :WhereIsItReleaseVersion' "$APP/Contents/Info.plist")" == "$VERSION" ]]
codesign --verify --deep --strict "$APP"
DB_SHA_AFTER="$(shasum -a 256 "$DB" | awk '{print $1}')"
[[ "$DB_SHA_BEFORE" == "$DB_SHA_AFTER" ]] ||
  { echo "Upgrade installer unexpectedly changed user SQLite bytes" >&2; exit 1; }

python3 tools/qa/upgrade_catalog_probe.py verify \
  --cli "$CLI" --db "$DB" --fixture-root "$FIXTURE" --snapshot "$BASELINE"
pkgutil --pkg-info "$RECEIPT" >/dev/null ||
  { echo "Expected package receipt after in-place upgrade" >&2; exit 1; }

# Simulate user uninstallation only on CI-owned test app; user DB/CTF remain.
sudo rm -rf -- "$APP"
sudo pkgutil --forget "$RECEIPT" >/dev/null
CREATED=false
[[ -f "$DB" && -f "$FIXTURE/external-user-ctf-placeholder.CTF" ]] ||
  { echo "External catalog or CTF was lost during cleanup" >&2; exit 1; }

OUT="$ROOT/dist/qa-upgrade/$ARCH"
mkdir -p "$OUT"
python3 - "$OUT/UPGRADE-MACOS.json" "$ARCH" "$OLD_SHA" "$NEW_SHA" "$(git rev-parse HEAD)" <<'PY'
import json, sys
from pathlib import Path
dest, arch, old, new, sha = sys.argv[1:]
data = {
    "format": "whereisit-rc2-to-rc3-upgrade-qa-v1",
    "platform": "macos-" + arch,
    "old": "1.0.0-rc.2", "new": "1.0.0-rc.3",
    "source_commit": sha,
    "old_package_sha256": old,
    "new_package_sha256": new,
    "bundle_build_before": 10002, "bundle_build_after": 10003,
    "installed_sqlite": "3.51.3",
    "category_and_query_parity": True,
    "external_user_catalog_bytes_untouched_by_installer": True,
    "external_ctf_placeholder_untouched": True,
    "uninstall_preserves_external_data": True,
    "only_generated_fixture_used": True,
    "status": "pass",
}
Path(dest).write_text(json.dumps(data, indent=2, sort_keys=True)+"\n", encoding="utf-8")
PY
echo "PASS: $ARCH RC2 -> RC3 in-place PKG upgrade, catalog parity, safe uninstall."
