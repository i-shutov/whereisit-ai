# Observed WhereIsIt Catalog 3.00 format

This document describes a clean-room implementation based on the user-supplied catalog and the vendor's text documentation. It is **not an official format specification**. It deliberately limits the compatibility claim to the layout actually inspected. The file extension is unrelated to GNU's Compact C Type Format.

## Evidence and validation

The inspected input is 144,766,574 bytes, SHA-256 `f9339112d0a2b3da114cb4f4efa026d4501da0933d294f01f02896fda08f33d5`.

The parser consumed and cross-checked:

- 44 disk blocks and one empty disk-group block.
- 2,805,080 file records.
- 240,947 directory/archive-container records.
- 3,046,027 entries in total.
- Every directory-table boundary, every file-range length, every file count, every aggregate encoded-name length, and contiguous coverage of the data area.

A full parse → rebuild → parse verified all entry names, parent indices, kinds, sizes, attributes, and date fields. The rebuilt file was **byte-for-byte identical** to the 144,766,574-byte input. The writer actually rebuilds catalog offset tables, disk blocks, file batches and folder records; it is not a function that returns the original file wholesale. Separate synthetic tests create new catalogs and disks, change names/sizes/descriptions, include names longer than 255 encoded bytes, CESU-8 supplementary characters, and files larger than 4 GiB.

**External compatibility remains pending.** Attempts to open the produced sample in the original Windows application were blocked by the execution environment's 32-bit/WoW64 incompatibility. Matching our own reader, even with exact reproduction of one large original file, does not prove that arbitrary new output will be accepted by every WhereIsIt version. The generic header for newly scanned disks has no private values or plugin state, but still needs validation in a supported Windows installation.

The vendor's supplied `Readme.txt` states that catalogs store textual data as UTF-8; the actual file also contains CESU-8-encoded UTF-16 surrogate pairs. The supplied `FAQ.txt` describes compression, a 2 GB catalog limit and Report Generator export. Neither text document publishes the binary record layout.

Primary external documentation: [WinCatalog's WhereIsIt export instructions](https://www.wincatalog.com/whereisit-export.html) describe the existing XML migration path: XML rather than XML (MIDAS), with catalog/disk/folder headers enabled. They also explain that report export omits disk serial numbers. This is background evidence, not the basis for the binary parser. No suitable published binary parser or official binary specification was found during the targeted search.

## Scope and safety boundaries

The reader is read-only, contains no `unsafe`, never executes source programs or catalog entries, and never extracts the files represented by a catalog. It reads the names and metadata only. Optional description/thumbnail records are retained as opaque bytes. Thumbnail data is not decoded or rendered.

`Limits` bounds input bytes, number of entries, number of disks and tree depth. Checked reads reject truncation and invalid offsets. Unknown file flags, unsupported header variants, overlapping ranges, unaccounted data, inconsistent counts, invalid encodings, invalid parents and excessive depth produce errors. The observed uncompressed layout is supported. Compression and encryption variants are rejected rather than guessed. The parser currently returns a materialized catalog; memory use is proportional to the catalog and its retained metadata. It is not a streaming parser.

The only supported disk group is the observed final **empty** group. Populated groups and multiple groups are rejected. An empty final group contains no disk-membership references, so it can be retained when disks are added. General group remapping is not implemented.

Categories, flags, icons and other unknown extended fields are preserved when present in unchanged records/blobs, but their semantics have not been identified. Native category assignment conversion is not claimed. Application categories should remain in the application's own database until their WhereIsIt mapping has been verified.

## Byte order and text

All multibyte integers below are little endian. Offsets inside a disk are relative to the start of that disk block. `0xffffffff` is used for absent file-range offsets/lengths.

Names are byte strings. Non-ASCII names commonly begin with UTF-8 BOM `ef bb bf`. BMP characters use UTF-8; some supplementary characters use CESU-8 surrogate pairs. The decoder accepts valid UTF-8 and paired CESU-8 surrogates and rejects malformed/unpaired sequences. The writer emits BOM plus CESU-8 for non-ASCII names. Original raw names are reused for unchanged records.

Descriptions can be inline or referenced in a per-folder metadata blob. Type 1 text normally includes a terminating NUL inside the declared payload length. Decoded descriptions are exposed for search. Unsupported descriptions stay preserved in opaque storage and are counted by `diagnostics()`; an empty search field must not be interpreted as proof that no source description exists.

## Catalog header

| Field | Layout |
|---|---|
| Signature | 13 bytes `Catalog 3.00\0` |
| Observed flags | Bytes `80 00 80` |
| Number of blocks | `u16`, includes the empty group |
| Display/order identifiers | `count × u16`; observed sequential IDs |
| Catalog name | `u16` byte length, followed by bytes |
| Observed settings | 5 bytes `06 b8 00 00 04` |
| Absolute block offsets | `count × u32` |
| Sentinel | `u32 = 0xffffffff` |
| Observed empty metadata area | 9 zero bytes |

No general category table or custom catalog-header variation is implemented. New output uses these observed numeric defaults.

## Disk header and data regions

| Relative offset | Meaning |
|---:|---|
| 0 | `u16 = 3`, disk record marker |
| 2 | `u16` observed flags `0x80` or `0xc0` |
| 4 | Reserved `u32`, observed zero |
| 8 | Entire disk-block byte length (`u32`) |
| 12 | Directory-table offset (`u32`) |
| 16 | Root-file range offset (`u32`) |
| 20 | Root-file range length (`u32`) |
| 24 | Root-file count (`u32`) |
| 28 | Total encoded file-name bytes in root range (`u32`) |
| 32 | Disk-name byte length (`u16`) |
| 34 | Disk-name bytes, then variable disk metadata |

The opaque header continues until the earliest referenced data range. The data area consists of the root file batch and each directory's metadata blob/file batch. The directory table runs to the exact end of the disk block.

Within the observed disk metadata, counters follow the name: normal file count (including archive containers), archived-member count and ordinary-directory count. Counts are rebuilt on export. Additional disk properties and optional plugin-state bytes remain opaque for imported disks. A newly created disk gets a sanitized numeric header with no source serial number, source path or enabled plugin records. Unknown filesystem information is left empty.

## Directory and archive-container records

A directory record contains:

1. `u16` marker; ordinary folders have bit `0x02`; archive containers additionally have `0x10`.
2. `u16` field flags.
3. `u8` tree depth. Root children have depth 1. Preorder and depth identify the parent.
4. `u16` name length and name bytes.
5. `u32` metadata-blob length.
6. Four `u32` values: file-range offset, file-range length, file count, aggregate file-name byte count.
7. `u64` stored aggregate size in Delphi Currency units: **bytes × 10,000**.
8. Optional explicit attribute byte when flag `0x20` is set; otherwise directory attribute `0x10`.
9. One DOS date/time word and optional extra date/time words according to flags `0x04` and `0x08`.
10. Optional extension list when flag `0x80` is set.

The referenced file range begins with the metadata blob. File records start after that blob. Blob bytes may include descriptions and thumbnails referenced by file extensions. Directory records can also have their own inline metadata. Both representations are preserved.

The timestamps are timezone-free packed DOS date/time words. The first observed word is treated as modification time. Additional/equality flags are decoded into the exposed time fields, but original timestamp bytes and compact flags are retained when unchanged. No UTC offset is inferred. Converting to Unix time is a caller policy and must record its timezone assumption.

## File records

A file record contains a `u16` flags word, one-byte short-name length and name bytes, followed by the variable metadata body and optional extension list.

Observed field flags:

| Flag | Observed effect |
|---:|---|
| `0x04`, `0x08` | Additional 32-bit date/time words |
| `0x10`, `0x20`, `0x30` | Compact attribute representation; no explicit attribute byte |
| neither `0x10` nor `0x20` | Explicit attribute byte |
| `0x40` | Compact/equality timestamp mode; no additional bytes |
| `0x80` | Extension list follows |
| `0x100` | Archived-member metadata layout, adds 4 bytes to the ordinary consumed length |
| `0x400` | Two high file-size bytes after the low `u32`; observed 48-bit size |
| `0x2000` | Preserved observed subtype flag, no additional bytes in this sample |

Ordinary records have a low `u32` file size after date fields and optionally a high `u16`. Archived members instead carry CRC/date/original-size/packed-size-related fields. The implementation exposes their modification date and original size and does not invent filesystem creation/access times for them. Unknown archive-related words are retained.

Long names use the first 255 bytes in the short-name field and extension type 12 for the remainder. UTF-8/CESU-8 sequences may cross that boundary, so decoding occurs after concatenation. The aggregate name-byte counter includes the continuation.

## Extension lists

An extension has:

- One-byte type; zero terminates the list.
- A packed `u32`, with payload length in its low 24 bits and flags in its high byte.
- An inline payload of that length, or a four-byte offset into the parent range's metadata blob when high-byte flag `0x08` is set.

Observed decoded types: 1 = description, 12 = filename continuation. Other types are preserved. A cap of 4,096 fields per extension list prevents unbounded extension iteration.

When an entry's description changes, the writer emits a new inline type-1 string, retaining other extension types. Changing a name regenerates its continuation record. Unchanged file records are reused verbatim. Folder pointers and hierarchy fields are rebuilt while unchanged metadata representations remain byte-preserved.

Moving an imported file between parents with external metadata references is not a supported interchange operation: its blob-relative unknown fields require semantic remapping. The application should not expose such a move until it resolves or migrates those references. Renaming or editing metadata in place, adding new scans, and re-exporting the original tree are supported by this implementation.

## Reproducible checks

From the workspace root:

```sh
cargo test -p ctf-format
cargo run --release -p ctf-format --example inspect -- /path/USB-drives.CTF
cargo run --release -p ctf-format --example verify -- /path/USB-drives.CTF /tmp/rebuilt.CTF
cargo run --release -p ctf-format --example summary -- /path/USB-drives.CTF /tmp/summary.json
```

Tests use generated data, not the user's catalog. Private source catalogs, rebuilt copies and their per-disk names are not included in this source package.

## Independent SQLite roundtrip validation

The full supplied catalog was imported into the application’s SQLite database and exported back to CTF. An independent audit using Python SHA-256 (separate from the exporter and its benchmark runner) confirmed equal byte lengths of 144,766,574 and matching SHA-256 `f9339112d0a2b3da114cb4f4efa026d4501da0933d294f01f02896fda08f33d5`. This establishes unchanged-catalog preservation through the database layer. See [independent export validation](QA_EXPORT.md) for the separate CSV/Parquet value checks. It does not establish original-application acceptance of arbitrary newly created CTF files.
