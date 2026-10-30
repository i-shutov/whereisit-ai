#!/usr/bin/env bash
# Portable negative/positive tests. Never exercise real disks or mount volumes.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
printf 'synthetic placeholder: never claim to have verified a real DMG\n' > "$TMP/fake.dmg"
cat > "$TMP/hdiutil" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
[[ "$#" -eq 2 && "$1" == verify && -f "$2" ]] || exit 99
count="$(cat "$TEST_COUNTER")"
count="$((count+1))"
printf '%s\n' "$count" > "$TEST_COUNTER"
case "$TEST_CASE" in
  success) exit 0 ;;
  busy_once)
    if (( count == 1 )); then
      echo 'hdiutil: verify: unable to recognize disk image. (Resource temporarily unavailable)' >&2
      exit 1
    fi
    exit 0 ;;
  always_busy)
    echo 'hdiutil: verify failed - Resource temporarily unavailable' >&2
    exit 1 ;;
  bad_checksum)
    echo 'hdiutil: verify failed - CRC checksum mismatch' >&2
    exit 1 ;;
  busy_plus_corrupt)
    echo 'hdiutil: verify failed - Resource temporarily unavailable, checksum mismatch' >&2
    exit 1 ;;
  generic)
    echo 'hdiutil: verify failed - Permission denied' >&2
    exit 1 ;;
  *) exit 88 ;;
esac
SH
chmod +x "$TMP/hdiutil"
export PATH="$TMP:$PATH" TEST_COUNTER="$TMP/counter" DMG_VERIFY_RETRY_DELAY=0
one() {
  local name="$1" expected="$2" calls="$3" actual count
  printf '0\n' > "$TEST_COUNTER"
  export TEST_CASE="$name"
  if bash "$ROOT/installers/macos/verify-dmg.sh" "$TMP/fake.dmg" > "$TMP/output" 2> "$TMP/error"; then
    actual=0
  else
    actual=$?
  fi
  count="$(cat "$TEST_COUNTER")"
  if [[ "$actual" != "$expected" || "$count" != "$calls" ]]; then
    cat "$TMP/error" >&2
    echo "FAIL: $name (status=$actual, calls=$count; expected $expected / $calls)" >&2
    exit 1
  fi
}
one success 0 1
one busy_once 0 2
one always_busy 1 4
one bad_checksum 1 1
one busy_plus_corrupt 1 1
one generic 1 1
echo 'PASS: six bounded DMG verification contracts; no false success on corrupt/unknown failures'
