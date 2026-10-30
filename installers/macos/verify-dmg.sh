#!/usr/bin/env bash
# Verify the *real* DMG. Only a verified OS resource-busy condition may retry.
# Bad checksum, corrupt image, malformed file and other hdiutil failures fail.
set -euo pipefail
[[ "$#" -eq 1 && -f "$1" ]] || { echo 'verify-dmg: expected an existing DMG file' >&2; exit 2; }
image="$1"
errors="$(mktemp)"
trap 'rm -f "$errors"' EXIT
# A 4-attempt bound avoids masking a persistent disk image/resource failure.
for attempt in 1 2 3 4; do
  if hdiutil verify "$image" >/dev/null 2>"$errors"; then
    printf 'PASS: hdiutil verified DMG (attempt %s/4)\n' "$attempt" >&2
    exit 0
  fi
  if (( attempt < 4 )) &&
     grep -Fq 'Resource temporarily unavailable' "$errors" &&
     ! grep -Eiq 'checksum|corrupt|CRC|truncat|size mismatch|invalid checksum' "$errors"; then
    printf 'WARN: transient macOS diskimages resource busy (attempt %s/4); retrying verify\n' "$attempt" >&2
    sleep "${DMG_VERIFY_RETRY_DELAY:-$attempt}"
    continue
  fi
  cat "$errors" >&2
  printf 'FAIL: hdiutil could not verify DMG on attempt %s/4\n' "$attempt" >&2
  exit 1
done
exit 1
