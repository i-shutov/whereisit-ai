# UX28 — candidate result → recorded folder → exact candidate return

Date: 2026-11-02. Branch: `ux/search-first-disk-locator-20261101`;
PR #57 (draft).

## Implemented native vertical

From **Find copies on other disks**, select a *file* on a different catalog
disk and use **Browse recorded containing folder** (Windows Ctrl+Shift+F;
macOS Command+Shift+F, toolbar even with the locator hidden). This is a
catalog-only navigation using exact `volume_id` + recorded `parent_id`,
never a historical drive letter or a live USB filesystem path.

The first **Back** restores the original candidate **source entry ID**, any
chosen candidate catalog disk, the candidate page offset and the view-only
`copies:` disk-register filter. The **second Back** restores the initial
global search (or duplicate/search scope) and its page. A changed CTF
generation is independently refused on the folder load **and** on return
to the candidate page: there is no saved result-page replay.

Normal UX27 search-result → recorded folder → Back still works. Fresh search,
disk selection, reinitialization or choosing a new source intentionally ends
the nested candidate excursion. There is no auto-open, playback, content
hashing, filename byte verification, deletion or change to linked CTF,
derived SQLite or private annotations.

## Synthetic manual regression matrix (Win10/11 and Mac ARM/Intel)

1. With three synthetic catalog disks ID 4/42/142 and identical visible disk
   labels, search globally for a recorded movie; select it and Find copies.
   Focus candidate disk ID **42**, move to page 2 if synthetic fixture is
   sufficiently populated. Select a candidate under a non-root parent.
2. Ctrl/Command+Shift+F: ensure catalog tree shows its exact recorded
   containing folder on ID 42, not disk 4 or 142. Test a root-level file
   and backslash-separated recorded paths too.
3. Back once: candidate mode, focused ID 42 and original offset restored,
   but **freshly loaded**, never cached or prematurely selectable rows.
   Back again: original global search/query/page. Verify keyboard, screen
   reader, locator collapsed at 880px, dense table at 1366×768, HiDPI.
4. Test pending page, folder selection, invalid/missing/self parent: refuse
   navigation without modifying history. While linked to a read-only
   *synthetic* CTF, modify a private **test fixture** between folder and
   Back; the new pinned request must fail closed, empty its page and
   preserve the source from app writes.
5. Start a new all-disk search or click a new disk while in the folder;
   Back must not unexpectedly resurrect the old candidate source. Neither
   USB presence nor byte-level identity is inferred from metadata.

## CI versus owner acceptance

Run exact-HEAD native Linux tests/fmt/Clippy + Windows and both Mac builds,
offline acceptance kit, integration source contract and installed RC2→RC3
upgrade tests. Only synthetic fixture data is permitted in GitHub Actions.
These checks cannot establish physical USB clone/plug/unplug correctness,
actual Win10/11 and Mac ARM/Intel GUI usability, original/historical CTF
compatibility, notarization, or real 3M-entry search p95 ≤150 ms.
Keep PR #57 draft/unmerged and release RC3 unsigned prerelease; no stable 1.0.
