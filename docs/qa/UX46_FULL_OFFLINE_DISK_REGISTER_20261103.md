# UX46 — Full offline physical disk register (2026-11-03)

**Draft integration:** PR #57, \`ux/search-first-disk-locator-20261101\`. This is a real clipboard action in native Win32 and SwiftUI clients, not a mockup. Source integration and synthetic CI are distinct from manual hardware/GUI acceptance.

## Operator task

A collection of ~40 disconnected USB disks should be retrievable by exact catalog ID and the owner's separately recorded sticker/shelf label, without opening each disk or relying on historical drive letters. **View → Copy FULL physical disk register** on Windows (Ctrl+Shift+4), or **File actions → Full disk register** on macOS (Command-Shift-4), copies one comprehensive local inventory regardless of current search, selected disk, register filter, \`page:\`, or \`copies:\`.

Each row is sorted by exact catalog ID, including the recorded label, optional owner-entered private sticker/shelf tag, historic count (if available), last independently OS-verified status, and a shared-sticker conflict warning. Header says **ALL REGISTERED DISKS**, unlike UX45 **CURRENT PAGE ONLY** distribution and the metadata-copy pickup list. Clipboard text stays on the user's machine; treat it as personal data if sharing/pasting elsewhere.

## Provenance and safety

- Linked CTF triggers a fresh \`ctf_volumes_pinned\` request and original generation SHA check at copy time. Windows rejects a reply from a superseded app generation; macOS also checks physical catalog scope and disk-ID set. Missing, stale or modified source means **no clipboard change**.
- Empty/missing/nonpositive/duplicate catalog disk IDs fail closed; no guessed disk label, mount root or historical drive letter becomes present-state evidence.
- Expired, incomplete or failed OS probes never retain a connected badge. Only explicit current UUID pairing and unique native OS observation may say connected, which **does not prove the file is still present**. Cloned filesystem UUIDs are possible.
- Original CTF remains read-only; SQLite/Parquet caches remain derivative, shelf tags and personal annotations stay in their own user-owned database. No content hashes or bytes are compared, and **no auto-deletion** is offered.

## Manual acceptance matrix — NOT CI evidence

1. **Win10/11, Mac ARM/Intel:** synthetic disks #4/#42 have a shared label and private shared-sticker conflict. Filter to a single disk or use UX45 \`page:\`; Copy Full must contain both, sorted with exact IDs and warnings. Confirm keyboard shortcuts and Narrator/VoiceOver discoverability, with no unrequested search or selection change.
2. **Hardware:** pair one current physical OS UUID, then unplug, sleep longer than the 12-second freshness window and simulate probe failure or cloned UUID. Re-copy the inventory and verify no stale green 'connected' state. The action never infers file existence.
3. **Linked CTF:** change a synthetic source after the register is shown and before Copy. The clipboard must remain unchanged; restore via a separately verified refresh, never trusting a prior SHA. Do not publish private source paths or hashes.
4. **Ergonomics:** test 880/1180/1366px and HiDPI. Menu action should remain usable without an additional toolbar button; inspect text on clipboard locally. Search, active disk and selected result must stay unchanged.
5. **Performance:** real owner hardware search p95 <=150ms and original app CTF compatibility remain independent release gates. Inventory copy timing on synthetic data is not a substitute.

**Release:** RC3 remains unsigned prerelease, PR #57 draft/unmerged. Manual Win10/11 Mac ARM/Intel USB/GUI/a11y, modified-original CTF and two provider-blocked historical CTFs, signing/notarization and real SLO still block stable 1.0. Earlier documented 44 real disks / 3,046,027 records evidence is genuine and has not been re-executed for UX46.
