# UX37 — live native filename lookup across offline disks (2026-11-03)

Draft integration: [PR #57](https://github.com/i-shutov/whereisit-ai/pull/57).
**Manual acceptance runbook — not evidence that these checks have run.**

## Operator workflow and scope

The primary Win32/SwiftUI file search now starts **global all-catalogued-disk**
lookup after an **idle pause of 450 ms**, for a **plain filename phrase** of at
least 2 Unicode characters and at most 256 UTF-8 bytes. Unicode letters/numbers,
spaces, period, hyphen, underscore and apostrophe are accepted. It uses the
existing asynchronous 250-row search pages, without mounting the source media.
At most the existing bounded page is displayed, not 3 million records.

Boolean keywords `AND/OR/NOT`, parentheses, `name:`, `ext:`, `size:`,
`type:video`, slash/backslash paths, short one-character terms and incomplete
field expressions **do not run as typeahead**. Press **Enter/Return** to execute
the complete explicit query. Deliberate **This disk** search is not replaced by
an implicit scope; live filename lookup is always across all catalog disks.
The list is catalog metadata, not live file presence, a media-title match or
content/hash identity. Search terms are never persisted into UI preferences.

## Windows 10/11 hands-on plan

1. With a **synthetic** multi-disk SQLite fixture, focus Ctrl+F, type
   `Ирония судьбы 1975.mkv` or `Star-Wars_1977` with a short pause. Verify
   one global query and correct page/physical-disk identity, irrespective of
   the previously selected disk. Verify a sub-450 ms pause does not dispatch.
2. Slowly enter `name:film`, `film OR clip`, `(film)`, `type:video`, and
   `size:100`. None should run until Enter. A pending timer must not
   submit an obsolete partial query after Enter.
3. Type a simple phrase, then immediately click a disk, navigate Back,
   use Find copies or toggle the video preset. Verify the programmatic
   edit guard cancels the timer; the new catalog scope stays authoritative.
4. Quickly replace one plain phrase with another. Delayed first results
   must not overwrite the second query. Stop/Retry, page forward/back,
   and source-generation change must preserve current-generation checks.
5. For **authorized test-only** linked CTF, mutate the test source between
   pages; every new page must reject a changed generation SHA. No SQLite
   fallback; original CTF, its derivative and private sidecars are unchanged.
6. Visually inspect keyboard, focus, spinner, search-scope caption, contrast
   and layout at 880/1100/1180/1366 px and 100/150/200% display scaling.

## macOS Intel and ARM64 hands-on plan

1. Type Cyrillic/Latin plain filename phrases at a steady rate, then pause.
   Verify only the current cancelled/restarted `.task(id: query)` dispatches
   and the search covers offline registered catalog disks without mounts.
2. Enter advanced expressions slowly, then press Return; test short terms.
   Observe that old results are not relabelled as a new query before a
   successful generation-matched page returns.
3. While a timer is pending, switch CTF, browse a disk, navigate candidate
   copies/folders/Back, or change focus. A previous query task must not
   override this new state. Test unplug/replug and stale OS pairing.
4. Verify VoiceOver's typeahead/Return hint, Command+F and Return focus,
   Command+Option paging, 1366px and HiDPI table/locator alignment.

## CI versus release acceptance

Synthetic source-contract and Windows compiled Rust unit tests, plus Windows
installer and macOS native builds, validate wiring and syntax. They do **not**
prove the manual GUI/USB/VoiceOver checks above. All linked CTF page operations
continue independent original-generation SHA verification. Physical connected
requires explicit pairing and a fresh complete unique OS identity probe;
historical drive letters are not trusted. No content hashes or auto-delete.

This feature is **not** a measured p95 improvement. The documented full-source
real p95 was 209.82–252.83 ms, above the 150 ms target. Separate genuine owner
CTF acceptance (44 disks, 3,046,027 entries) remains valid under its earlier
scope and must not be relabelled synthetic. Before stable 1.0: original-app
modified CTF readback, two already-provided historical CTFs whose raw bytes
are blocked by the provider, manual Win10/11 and Mac Intel/ARM GUI and hardware,
signing/notarization, and observed real p95 <=150 ms. RC3 stays an unsigned
prerelease; PR remains draft and unmerged. No private data enter public CI.
