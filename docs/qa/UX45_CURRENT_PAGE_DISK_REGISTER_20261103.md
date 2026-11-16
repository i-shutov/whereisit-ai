# UX45 native current-page disk register — manual acceptance

Run on Windows 10/11 and macOS ARM64/Intel (not yet executed in CI). Open
synthetic local catalog with 3+ disks including duplicate labels but distinct
exact IDs. Search across all disks and use View → **Show disks on CURRENT
results page** / Ctrl+Shift+6 (Windows) or register **Page** / Command+Shift+6
(macOS). The register must show only exact IDs among the <=250 currently loaded
results, with accurate hit counts and explicit CURRENT PAGE ONLY. Order by
count descending then disk ID. Check shelf/sticker conflicts and independent
OS status stay distinct from recorded file presence; do not trust a drive
letter. Only a fresh complete OS identity check and explicit pairing may say
connected.

Navigate next/previous pages. During SHA recheck, disk membership must clear
immediately; on fresh success it must recompute. Stop/retry, invalid linked CTF
generation, stale async page reply and catalog switch must never show old disk
IDs. Ctrl+L / Command+L clears the view filter; search expression, selection,
underlying scope and copying candidates must not change. Inspect at 880,
1180, 1366px plus HiDPI and VoiceOver/keyboard-only. Test unplug/replug and
clone UUID conflicts on actual devices. This page map is NOT a distribution
of all global query results and does not establish content hashes/equality,
or justify deletion.

Do not upload owner CTFs, SQLite, private disk labels/paths/SHA, device IDs or
screenshots containing them to public CI. Store real hardware evidence in
private QA only. Existing real 44 disk / 3,046,027 row acceptance remains real,
not synthetic. Historical CTF raw bytes are provider-blocked; do not request
again. Unsigned RC3 remains prerelease; stable 1.0 is blocked.
