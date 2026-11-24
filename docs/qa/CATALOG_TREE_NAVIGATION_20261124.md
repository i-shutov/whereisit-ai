# Catalog navigation: owner correction and observed WhereIsIt workflow

24 November 2026. This is an explicit owner UX correction, supplementing PR58.
The owner rejected making an isolated global filename list the primary catalog
view and does not want a mandatory global name sort. Preserve the PR58
composition while implementing the catalog's disk/tree context.

## Observed original application

The installed original WhereIsIt was exercised through the Windows GUI with a
separate read-only working copy. Observed actions: open catalog; select a disk;
expand its tree; select a folder; select a file; double-click a nested folder;
Alt+Left back. No cataloged file was launched and no catalog was saved. Original,
frozen working copy and navigation-copy byte hashes remained unchanged. Raw
screenshots, source paths, labels and catalog rows remain private.

The catalog root displays disks in the central table and as roots in the left
tree. A single disk selection shows its immediate root children. Tree expansion
is distinct from navigation. Selecting a folder shows only its direct children;
the visible location retains disk and folder ancestry. Entering a folder from
the central table synchronizes the left tree. Back returns to the parent context
with the child folder selected and previously expanded tree branches retained.
Selecting a file preserves its disk, parent tree location and visible path.
Search is a separately requested operation; opening a catalog does not implicitly
search every file.

## Implementation contract

1. Start at a catalog/disk overview. Do not issue an empty all-disk search after
   loading volumes. A single disk selection browses that disk's root.
2. Extend the existing left disk rail with an expandable folder tree. Fetch child
   pages lazily; expansion alone does not change the current central folder.
   Retain branch expansion across navigation. Never present a partial page as a
   complete child list; offer explicit continuation.
3. The central browse table contains immediate folder children, with local
   directory ordering. Double-click/Enter on a container enters it. A file
   selection opens metadata context only and never executes the cataloged file.
4. Render a clickable location `Catalog > disk label > ancestor folders` backed
   by stable volume/entry IDs. Up means the structural parent; Back means history
   and restores scope, query, page, selection and scrolling.
5. Search remains explicit. A nonempty search can cover the selected context or
   the whole catalog. Clearing a query returns to its browsing context. The
   catalog action/crumb returns to the disk overview, not a flattened empty search.
6. Search results retain disk and path. Opening a result's containing folder
   reveals/selects that precise entry on the correct child page and expands its
   ancestry. Duplicate names on different disks must not redirect by name.
7. Keep the PR58 header, rail, central table and inspector. Implement/test the
   interaction in `apps/ui-prototype` first, verify it visually, then port the
   same behavior to Rust/egui. No web runtime enters the product.

## Backend boundary

Existing `children` and `children_focused` provide direct child pages and focused
paging. Add a read-only ancestors operation for stable, root-to-entry breadcrumb
IDs. Direct CTF requests retain source/generation checks; native catalogs and
legacy linked mirrors use their existing read-only/pinned boundaries. Do not
infer ancestor IDs by splitting displayed path strings.

This change does not require a global flattened filename ordering, SQLite
conversion, a replacement query engine or new catalog-wide sorting work. Search
executor optimization is subordinate to the primary browsing workflow.

## Acceptance

Use synthetic multiple disks with duplicate folder/file names, nested Unicode
folders, archives and a directory exceeding one page. Verify catalog overview,
single-click disk browsing, independent expansion, tree/table synchronization,
crumbs/Up/Back, search-to-containing-folder reveal, page/selection restoration,
catalog switching and cancellation/source-generation safety. Then observe the
native implementation on a protected full working copy. Record actual tested
builds separately from installer/release acceptance. `stable_ready=false` and
published releases remain unchanged.

## Backend checkpoint

The new `entry_ancestors`, `ctf_preview_ancestors` and `ctf_ancestors_pinned`
operations return `{volume_id, items:[{id,parent_id,name,kind}]}` in root-to-self
order. The native SQLite operation opens an existing catalog read-only; a
missing path is rejected without creating a database. Direct CTF and pinned
mirror operations retain their existing source/generation verification.
Cycles, excessive depth, non-container parents, missing entries and crossed
volume boundaries are rejected. Breadcrumb IDs are never derived from names.

The Windows release-mode service suite with the optional DuckDB feature passed
73 tests. New generated cases cover SQLite/direct/pinned response parity,
Unicode/nested archives, invalid ancestry and source preservation. Native UI
integration and exact candidate CI remain separate checks.

## PR58 prototype checkpoint

The existing prototype composition now opens to 60 generated disk rows and a
single expandable rail. No initial all-file query is issued. The tree fills the
rail, overview counters are labeled as entries, and the overview does not invent
a selected disk inspector. Search is submitted explicitly; clearing it returns
to the remembered browsing scope.

Actual browser interaction verified catalog overview, single-click disk root,
independent tree expansion/collapse, nested folder navigation from tree and
table, synchronized ancestry, file context, structural Up and Back restoring
the child-folder selection. A global query with duplicate filenames on different
generated disks revealed the chosen result on the correct disk, with the full
ancestor chain and file highlighted. Disk-scoped search followed by an empty
query returned to that disk's direct root children.

TypeScript lint, 16 prototype runtime utility tests and the Vite production build
passed. Vite reports its existing `__dirname`/future-loader warning. The prototype
contains simulated metadata only; it is not a native or installed-product test.

The follow-up passes 21 utility tests, lint and production build. Stable folder
IDs now survive map/search workspace jumps. Actual browser interaction verified
folder browse -> map of that folder -> empty-query jump back to its immediate
children, with the same expanded ancestor tree and breadcrumb. The upper map
action was corrected after this check exposed that it used only search scope.
Generated tests also cover an ancestor beyond 250 siblings and distinguish
known folder IDs from legacy path-only search targets.

## Native integration checkpoint (in progress)

The first isolated Windows release build compiled. Actual native interaction on
60 generated disks verified disk rows in the central overview, single-click
root browsing, Unicode folder entry, immediate children, synchronized expanded
ancestors, structural Up and Back restoring a selected child. A duplicate-name
search result on disk 30 revealed that exact file and its breadcrumb without
opening the same-named file on disk 1. Source editing was not exercised.

This intermediate GUI pass exposed missing Enter activation and a tree viewport
that stayed on an older expanded disk after revealing a result on another disk.
These must be fixed and checked on the final candidate; the intermediate build
is not complete acceptance. UI automation's element coordinates also proved
unreliable after maximizing; screenshot-relative input was used for verification.

CI caught eager unsigned subtraction in the generated deep-parent test fixture;
the fixture now uses checked subtraction. All 69 default-feature service unit
tests passed locally in debug mode, including depth rejection. Formatting is
included in the next native checkpoint. No source-catalog hash changed.

After fixing the findings, the isolated Windows release-mode desktop suite
passed all 103 tests. Coverage includes initial overview without any query,
disk-root browsing, branch-query clear, stale ancestor replies, late ancestors,
focused file pages and Back/Up, selected-container Enter without file execution,
map scope/category preservation, imported catalog browsing before edit/export,
and the existing narrow-window Path regression. `cargo fmt --all -- --check`
and `git diff --check` passed. Final native GUI verification remains pending
until the rebuilt executable is exercised.

The `989042d` isolated executable was exercised again. Enter opened a selected
Unicode container, disk 30 focused reveal selected the exact duplicate-name
file on child page 251-302 and scrolled the central table to it, Back restored
the branch query and selected result, and the explicit All action returned 60
generated disk results. Cold lazy sibling loading exposed a second scroll
race: the pinned ancestor's initial position moved after 250 siblings arrived.
The reveal target now survives pending child requests; an egui render/reflow
regression was added. The 104-test desktop suite passed with this fix.

On a protected read-only full working copy, native GUI verified the 44-disk,
3,046,133-entry overview, a large disk's six direct root children, a nested
folder's 43 direct children, synchronized tree/breadcrumbs, Enter, Back with
selected child restoration, and a file inspector retaining disk and recorded
path. Original, frozen copy and navigation copy byte hashes still match the
private baseline. No filenames, paths or screenshots from that catalog are
included here. Final cold-tree scroll retest remains pending on the rebuilt fix.

The actual results-table synthetic egui input test confirms one click selects a
folder and two timed pointer press/release pairs open it. All 105 isolated
desktop tests passed; strict all-target desktop Clippy also passed after fixing
the test input initializer. The Windows automation double-click abstraction
did not establish physical double-click acceptance, so it is not claimed here.

Final cold-tree GUI retest is BLOCKED by the interactive desktop environment:
Windows `GetCursorPos` returns access denied, although session 2 is Active and
the new synthetic window's accessibility text remains readable. Capture is
stale from the preceding full-copy window. Fresh window selection and explicit
activation did not recover input. The owner must return the RDP session to an
unlocked ordinary desktop before input is retried. This is not a product PASS
or a proven application defect. No Windows security settings were changed.
