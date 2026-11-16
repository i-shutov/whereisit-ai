# UX & Architecture Spec: 60-USB Video Catalog & `egui` Ergonomics

> **Implementation status — reference prototype, NOT native egui.** The code in `apps/ui-prototype/`
> is a browser-based React/TypeScript + Express in-memory simulator using generated fixtures.
> `main` currently contains Win32 and SwiftUI/AppKit production frontends; the real cross-platform
> Rust/egui app is **not yet implemented**. Port UX to a separate Rust `egui` app rather than
> replacing Win32 calls in `apps/windows/src/native.rs` directly.
>
> **Safety and evidence:** No physical USB/CTF SQLite integration, byte-exact duplicate proof,
> media-header decoder, external-copy existence proof or measured 5 TB scan benchmark exists in
> this prototype. Size/time/storage numbers below are **unverified design hypotheses**, not
> product metrics. No destructive deletion or cleanup scripts are permitted in this prototype.
Interactive reference prototype: `apps/ui-prototype/` (React + TypeScript + Express API).

## 1. Key Ergonomic Principles (`egui` 1:1 Mapping)

Every widget and interaction in `apps/ui-prototype` maps directly to standard `egui` / `egui_extras` primitives for future implementation in a new Rust/egui frontend (`apps/desktop-egui`), NOT as a direct patch of Win32 control code:

- **Left Disk Panel (`egui::SidePanel::left`)**:
  - Compact single-line rows (`#ID` + USB Label left, indexed TB right, thin utilization bar below — no shelf/rack rows).
  - Pinned root item **`All Catalog Disks (60)`** at the top (classic *WhereIsIt?* model). Clicking any disk scopes search to that disk; clicking root returns to all 60 drives.
  - **Fast Inline Rename (`F2`, double-click, or `✎`)**: replaces row label with `egui::TextEdit::singleline` (`Enter` commits `UPDATE volumes SET label = ?`, `Esc` cancels).
  - Hover/active buttons on each row: `✎` (fast rename), `⚙` (Disk Properties modal), `↻` (Scan Disk modal).
- **Modal Dialogs (`egui::Window::new(...).collapsible(false).resizable(false)`)**:
  - **Disk Properties (`DiskPropertiesModal.tsx`)**: edit catalog `label`, enclosure sticker `diskCode` (`USB-01..USB-60`), `hardwareModel`, `capacityBytes`, `filesystem`, and free-form `note` (classic *Disk Description*). Shows telemetry (`used/capacity`, `entryCount`, `videoFileCount`, active scan plugins, `serialNumber`) and a button to launch Rescan for that drive.
  - **Scan USB Disk (`ScanDiskModal.tsx`)**: opened from `+ Scan Disk…` (`Ctrl+N`) or any disk's `↻` button. Supports adding `#61` or updating `#1..#60` with toggleable WhereIsIt-style metadata plugins and a live SQLite DB overhead + 5 TB scan time budget.
- **Main Results Grid (`egui_extras::TableBuilder`) + Docked Bottom Inspector (`egui::TopBottomPanel::bottom`)**:
  - Dedicated `Video (Duration · Stream)` column (`02:46:34 · 2160p HEVC`).
  - Bottom panel displays active USB metadata, `F2` / `Props` / `Scan` triggers, exact file size + video header summary + copyable relative path, and cross-disk copy actions.

## 2. Proposed Light Video Header Plugin (unverified storage/performance estimates)

Traditional WhereIsIt plugins bloated `.CTF` databases by storing frame thumbnails and verbose text dumps. For a 99% video archive across 60 USB drives:

1. **Base FS Stat (`always on`, `92 B/row`, `~5s / 5 TB`)**: `name`, `parent_id`, `size_i64`, `mtime_i64`, `ext`.
2. **Video Header Light (`on by default`, `+16 B/file` = `+1.6 MB / 100k videos`, `+2.5s / 5 TB`)**:
   - Reads only the first **16–64 KB** of MKV/MP4/M2TS/AVI container headers (zero frame decoding, zero thumbnails).
   - Stores compact binary/typed columns: `duration_sec (u32)`, `width (u16)`, `height (u16)`, `codec (u8)`, `audio_tracks (u8)`, `bitrate_kbps (u32)`.
3. **Sparse `3×64 KB` Fingerprint (`optional`, `+8 B/file`, `+3.5s / 5 TB`)**: hashes head + middle + tail `64 KB` slices for files >= 10 MB instead of reading 5 TB for full SHA-256 (~10 hours over USB 3.0).
4. **Archive Inner Listings (`.zip/.rar/.7z`, `off by default`)**: preserves minimal DB row count.

## 3. Four Video Duplicate Detection Modes

Because ~99% of cataloged items are cinema/video files, byte-exact matching alone misses remuxes and multi-resolution upgrades:

1. **Multi-Quality Rips (`4K vs 1080p vs 720p`)**: groups identical film/work keys (`workKey`) or normalized title + matching runtime across resolutions so forgotten `1080p`/`720p` rips on older USBs can be cleaned after archiving a `2160p UHD` release.
2. **Same Video Stream / Remuxes (`duration ±1s + resolution + codec`)**: detects the same encoded video stream inside `.mkv` vs `.m2ts` BDMV containers or with extra audio tracks where byte size and SHA-256 differ.
3. **Exact Byte Copies (`exact_bytes` / `3×64 KB`)**: catches byte-identical files across disks, including renamed copies.
4. **Same Filename, Different Size (`same_name`)**: flags colliding filenames with divergent byte sizes.

## 4. Ephemeral Local PC Mode (`LocalDiskAnalyzer.tsx`, review-only)

Read-only DaisyDisk/TreeSize-style inspection of currently attached local folders/drives on the host PC (`Ctrl+N` archive scanning vs Local PC ephemeral tab) with zero writes to the 60-USB catalog, plus live read-only overlap check (`POST /api/catalog/match-local`) showing which local files are already backed up on offline USB drives.

### Evidence requirements before enabling real disk operations

- Benchmark container parsers and data-storage overhead on a controlled fixture; missing or non-leading container headers must fail closed.
- Distinguish **metadata candidates**, **matching sparse samples**, and **content-verified copies**. Duration+codec or size alone never proves video-stream or file identity.
- Real USB identity must be validated by OS discovery, explicit user binding and duplicate-ID conflict checks; never infer from historic drive letters.
- Keep actual source CTF read-only and do not claim original-application compatibility without a physical original-app test.
- No deletion or generated executable cleanup scripts until independent end-to-end recovery/backup evidence and explicit separate acceptance. CSV review only.
