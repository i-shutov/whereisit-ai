# UX35 — restore the exact selected film on Back

Date: 2026-11-03; existing draft PR #57. Implemented in native Windows
Win32 and macOS SwiftUI, not a conceptual mockup.

## Exercise the full two-level workflow

1. With a **synthetic** multiple-disk catalog, search globally, sort the
   loaded page (at most 250 entries), advance to page 2 and select a film.
2. Find name+recorded-size candidates. Choose one on another registered
   disk. Browse its *recorded catalog parent folder*. First Back re-queries
   the candidate page and reselects that exact candidate; second Back
   re-queries the original search and reselects the original film.
3. Repeat with exact-filename, exact-size and direct recorded-folder
   excursions. Verify search offset, selected disk ID, row highlight and
   inspector, plus focus for arrow-key continuation.
4. Windows: ListView restores selected+focused state and ENSUREVISIBLE after
   the successful page. macOS: Table selection and keyboard focus restore
   after the published verified-page event. Test 880px collapsed inspector,
   1366x768, HiDPI, Narrator and VoiceOver.
5. Change query/source, Stop waiting, or invalidate the CTF generation while
   the Back request is pending: previous selection must remain empty.
   Missing entry, duplicate exact pair or changed disk ID must never be
   mapped to a plausible but unrelated file.

## Automated and remaining acceptance

Pure Rust tests validate exact entry+catalog disk identity, duplicate rows,
missing ID, reordered page, generation mismatch and unready page. Python
cross-native source wiring and all four native OS Actions builds must pass at
the **new exact HEAD**. These synthetic gates do NOT certify physical USB
presence, manual GUI, or real 3-million-record performance. Linked CTF stays
strictly read-only with a fresh generation SHA check on each returned page;
no cache/annotation/personal CTF write. No copy equality, file byte/hash or
automatic deletion claims.

Still blocked: real Win10/11 and Mac ARM/Intel native GUI/USB testing,
original modified CTF, two historical provider-blocked CTFs, signing/
notarization and measured p95 <=150ms. RC3 unsigned prerelease; stable 1.0
not authorized. Do not merge this draft PR.
