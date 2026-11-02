# UX31 — Contextual selected-file actions and compact macOS search toolbar
Date: 2026-11-02 • Draft PR #57 • Manual acceptance **not completed**

## Operator task

With about 40 normally disconnected USB disks, search globally for a film,
select an actual result and compare candidate locations without first expanding
the third inspector. Actions target the selected catalog row, not a guessed
Windows drive letter or macOS path.

**Windows Win32:** right-click a result row or press **Shift+F10** with a
selected result. The native menu offers: browse its recorded catalog parent,
find same-name + recorded-size candidates on *other catalog disks*, search
exact filename at any size, search same recorded byte size regardless of name,
show its exact catalog disk ID in the left register, and copy the catalog
retrieval note or disk pickup card. Clicking another row must select that exact
row before showing its actions. Right-click blank table area or attempt while a
page is loading must not reuse the previous selection. Context enablement
checks the exact result index, catalog disk ID and current ready page. Existing
source-generation SHA validation still applies to every linked CTF read/copy.

**macOS SwiftUI:** secondary click/control-click the result table (first select
the row, then invoke context actions) to see the same catalog-only
navigation/candidate/retrieval actions. The toolbar now groups Folder, Disk
card, Copy OS root and Finder in a visible **File actions** menu; their
Command-Shift-F/L/H/B shortcuts remain even with the locator collapsed.
The visible third-pane OS root buttons have independent VoiceOver labels:
copying a root is not opening Finder.

## Manual native acceptance — required on target hardware

1. At 880px collapsed / 1366×768 / Retina or Windows 150%-200%, search
   across all registered disks; verify result table, register and shortcuts
   remain usable without control overlap. Inspect both light and dark themes.
2. Select a file on catalog disk **42** while register also contains disks
   **4** and **142** with the same label. Context actions must show/reveal
   exact ID 42; switch to a file on 142 before right-click to confirm
   selection changes. No historical letter or OS path may be assumed.
3. During a deliberately slow linked CTF page request, open context menu:
   no old file action may be executable. Stop waiting / retry then check
   the new selected row. Replace the linked source between selection and
   clipboard: the generation check must refuse the copy.
4. Test folder and file / null and zero sizes. Candidate results must
   exclude the source *catalog disk*. The "same size" action is disabled
   for size zero and for folders; no content hashes or equivalence claimed.
5. Test keyboard only: Win Shift+F10 / arrow navigation / Enter / Escape,
   Mac Control-click and Command-Shift-F/L/H/B when third pane hidden.
   Check narrator / VoiceOver labels, focus and tab order.
6. Unplug, replug and clone a paired USB filesystem UUID; only explicitly
   paired and uniquely OS-confirmed current mounts may show connected.
   Source catalog items do not become verified live files. Root actions
   continue to require a fresh CTF SHA plus two OS identity probes.

## Automated evidence and safety

Compiled Windows unit regression covers exact 4/42/142 disk IDs, row mapping,
disabled folder/zero-size actions, invalid row, incomplete/pending pages and
missing catalog disk. Dual-native Python source contracts and native
Windows/macOS ARM64/Intel CI guard actual wiring; CI is **not visual QA**.
Tests use synthetic values only. No user CTF, SQLite, private names/paths or
source hashes published. CTF remains strictly read-only; personal labels and
annotations stay in separate user databases. No auto-deletion. Do not
promote RC3 unsigned prerelease or merge draft PR #57 until release gates pass.
