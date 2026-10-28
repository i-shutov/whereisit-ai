# Native desktop applications

WhereIsIt AI provides two native desktop frontends over the same Rust SQLite catalog engine. The Windows application uses Win32 common controls directly from Rust. The macOS application uses SwiftUI and AppKit panels with a small C ABI boundary. Neither application embeds a browser or a JavaScript runtime.

## Compact catalog workflow

The interface is a disk list beside a dense, paginated file table. The toolbar opens or creates a catalog, scans a disk or directory, imports CTF, and exports the active scope. The expression field shows Boolean syntax and accepts the engine's `AND`, `OR`, `NOT`, parentheses, and `name:`, `path:`, `ext:`, `size:`, `category:` filters. The table loads 250 entries per page. Search uses `search_fast` without computing a full match count: the status shows the loaded range and “more available”, and Next follows the returned `has_more` flag. Search covers all disks when “All disks / global search” is selected; selecting a disk limits the scope. Double-click a directory to enter it; Back returns to its previous folder or leaves search results. The original disk may be offline: file rows describe catalog metadata and are not file launchers. Dates use readable local dates/times, and sizes use compact human-readable units; exports retain exact numeric metadata.

Duplicates uses the existing database, with Name + size, Size, or Name matching. Results show a Matches column and explicitly identify metadata candidates whose content has not been verified. The scan operation alone reads the selected original filesystem. CSV and Parquet export the current branch recursively, current search, or current duplicate candidates. CTF exports the entire catalog, as labeled in the format selector. Import/export errors from the core are surfaced in a native error dialog.

Exports use the last applied query and duplicate matching mode, even while the user edits a new expression or changes the matching selector. All-disks export uses an unrestricted search scope. An existing export file is overwritten only after the native Save dialog has accepted the choice and its overwrite confirmation.

The lower sidebar lists categories. Type a name and use `+` to create a category. Select a category and choose Filter to apply it to the current disk/global search scope. Select an entry and category, then Assign category / Assign selected to attach it. These controls call the engine's real `categories`, `create_category`, and `assign_category` operations; category filtering populates the visible expression field so it can be combined with other Boolean filters.

Every database request runs on a worker thread, including initialization, enumeration, search, duplicate queries, scan, import, and export. Windows transfers replies over an MPSC channel drained by the window timer. macOS transfers immutable response data to the main dispatch queue before updating the observable model. A monotonically increasing generation discards stale table results after a newer search or navigation. Native controls are touched only on the UI thread. A running scan has a cancellation token; import and export are shown as busy operations without a misleading cancel control. SQLite persistence and migrations belong to the engine.

## Windows 10+ x64

Install Rust 1.90+ and Visual Studio Build Tools with the desktop C++ workload and Windows SDK, then run from the repository root:

```powershell
./apps/windows/build.ps1
./target/x86_64-pc-windows-msvc/release/atlas-windows.exe
./target/x86_64-pc-windows-msvc/release/atlas-windows.exe C:\Catalogs\Media.atlas.sqlite
```

The default catalog is `%LOCALAPPDATA%\AtlasCatalog\Catalog.atlas.sqlite`. The most recently opened path is saved beside it in `catalog-path.txt`. A database path passed as the first argument takes precedence. The native controls support Tab navigation, Alt accelerators, Enter in the query field, and Enter on a selected directory. The file list uses report columns, full-row selection, native accessibility, and window resizing. The window requires at least 880 × 430 logical pixels.

For a Linux cross-check when the Windows GNU target and MinGW C compiler are installed:

```sh
rustup target add x86_64-pc-windows-gnu
cargo check -p atlas-windows --target x86_64-pc-windows-gnu
cargo build --release -p atlas-windows --target x86_64-pc-windows-gnu
```

## macOS 13+ (Apple Silicon and Intel)

Install Xcode Command Line Tools and Rust 1.90+, then on the target Mac:

```sh
bash apps/macos/build.sh
open 'dist/macos/WhereIsIt AI.app'
'dist/macos/WhereIsIt AI.app/Contents/MacOS/AtlasCatalog' /path/to/Media.atlas.sqlite
```

The script builds the shared Rust library, builds the Swift package, assembles an app bundle with its private library, fixes the library install name, and applies an ad-hoc signature. It builds for the host architecture. Build separately on Apple Silicon and Intel to get a native binary for each; a release pipeline can combine corresponding artifacts with `lipo` after both architecture builds. A distributable download requires a Developer ID signature and notarization. Neither a universal binary nor notarization is implied by the local script.

The default catalog is `~/Library/Application Support/AtlasCatalog/Catalog.atlas.sqlite`; the selected path is stored in `UserDefaults`. A database path passed as the first argument takes precedence. The sidebar and table are native SwiftUI controls; folder, open and save dialogs are AppKit `NSOpenPanel`/`NSSavePanel`. The app uses a standard resizable split view. The package requires the sibling Rust `atlas-ffi` library to be built first. Its `CAtlas` system module declares:

```c
uint32_t atlas_abi_version(void); // 1
char *atlas_call(const char *request_utf8);
void atlas_string_free(char *response_utf8);
```

The library owns returned strings; Swift copies each response into `Data` and calls `atlas_string_free` exactly once. The frontend rejects an ABI version other than 1. JSON calls use the same `{op,db,...}` request and `{ok,data,error}` envelope as the CLI/service. Unknown or null optional metadata is decoded safely.

## Verification and platform limits

`cargo check -p atlas-windows --target x86_64-pc-windows-gnu` passed in the Linux development environment with Rust 1.90 and the MinGW C compiler, validating the actual Windows source and Win32 declarations.

Linux cannot run or typecheck the AppKit/SwiftUI application against the macOS SDK. The macOS source and build script therefore require a native Mac build and smoke test before release. Required Mac smoke scenarios are first launch, opening a copied catalog, scanning a Unicode directory, paginated folder navigation, Boolean search, cancellation, all three export formats, CTF import, category creation/filter/assignment, and native error dialogs. Actual Windows keyboard, dialog, DPI, and accessibility behavior still needs a Windows or Wine GUI smoke test.

Official platform references used for implementation: [Microsoft common-control ListView creation](https://learn.microsoft.com/en-us/windows/win32/controls/using-list-view-controls), [Microsoft windows-sys bindings](https://github.com/microsoft/windows-rs), [Apple NSOpenPanel](https://developer.apple.com/documentation/appkit/nsopenpanel), and [Apple NSSavePanel](https://developer.apple.com/documentation/appkit/nssavepanel).
