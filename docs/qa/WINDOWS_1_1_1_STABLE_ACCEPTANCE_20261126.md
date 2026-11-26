# Windows 1.1.1 stable readiness — 26 November 2026

**PASS for the tested Windows candidate; final publication remains conditional on
verification of the rebuilt exact-main installer.** The owner requested release
of Windows 1.1.1. Functional and data-safety acceptance is complete for the
exercised scope, with no remaining confirmed P0/P1. Setting readiness does not
retroactively certify a different installer. Existing published tags and assets
remain immutable.

## Exact evidence and scope

Windows x64 Rust/egui, unsigned. USB pairing and bound-media rescan stay disabled;
ordinary metadata scanning is supported. macOS/Linux product releases remain
separate; their successful CI is compile/test coverage. Windows 10 coverage is
historical owner-confirmed coverage, not an exact 1.1.1 artifact PASS.

The tested standard product candidate was source
`3cf2d36fe2929858cd768ef2d080a82d1725ca5a`, hosted artifact `15143743307` from native
CI run `45873945126`. Archive checksum, installer checksum and provenance were
verified; installed GUI and CLI bytes matched the product provenance. The
[installed candidate report](WINDOWS_1_1_1_INSTALLED_CANDIDATE_20261126.md) records
the exact local and hosted checks separately. PR #101 subsequently passed all
15 checks at `975a174`; integration main `8f95d639` has the same Git tree.

## Installed lifecycle and GUI — PASS

Actual interactive Windows 11 Administrator RDP testing covered upgrade from
installed 1.1.0, launch, restart, uninstall and reinstall using the local standard
candidate. Two generated external SQLite catalogs, a real application-format
preferences store, an external invalid CTF preservation marker and the existing
default user store stayed byte-identical. Notes, tags, category and preferences
retained their semantics. The invalid marker is preservation evidence only.

The exact hosted product installer was separately installed and its binaries
verified. Its GUI directly opened a protected full CTF copy with 44 volumes and
3,046,133 records, returned search pages 1–250 and 251–500, closed normally and
restarted with saved disk/shelf properties. Native SQLite analytics highlights
for description, tag and default text each showed the expected one file and
11 bytes, without the previous missing-column SQL error. The earlier actual
60-disk GUI and cancellation/retry evidence remains linked through PR #100.

## Original WhereIsIt in both directions — PASS

The installed candidate edited one folder description in a separate copy of a
verified full derived SQLite, then exported to a new CTF without overwrite or
lossy export. Original WhereIsIt 2014.0.0.220 opened that file and its actual GUI
showed the complete inserted Unicode note.

For the reverse direction, original WhereIsIt changed that note on another copy
and used Save As to create a distinct new CTF. The provenance-pinned hosted
installed 1.1.1 CLI imported it into a new SQLite. Validation and quick_check
passed: 44 volumes and 3,046,133 entries. All 3,046,133 logical entry rows were
compared across ten semantic fields, resolving parent paths and excluding
import-local row IDs/timestamps. Exactly the intended one description differed;
all other compared fields and semantic volume metadata matched. Import took
181.057 seconds; validation took 30.878 seconds. These are explicit conversion
operations and are not required for direct read-only GUI opening/search.

Original, master and protected working copies retained their baseline bytes.
The vendor-saved input and comparison SQLite were also unchanged by verification.
This accepts the exercised supported layout, not every possible CTF variant.
It is a ten-field semantic comparison, not a new fifteen-field CSV/Parquet test.

## Search measurements and limitations

At the unchanged product-code checkpoint `24d7e77`, two independent series of
30 samples in each of ten classes met p95 ≤150 ms on the full catalog: worst
p95 122.112 ms; ordinary text about 29 ms. Opening through first search was
1.273 / 1.217 seconds. These are service measurements with uncontrolled OS cache
on a four-logical-CPU Windows 11 host, not an installed-final-build benchmark or
an arbitrary-query/hardware guarantee. Peak opening working set remains about
863 MiB, with peak commit about 1.06 GiB. Reducing that peak remains follow-up
work; cancellation remains available for expensive expressions. See the
[measurement report](CTF_TEXT_SCAN_20261126.md).

## Final publication sequence

Readiness changes release records and the publication-branch guard, not Rust
product code. The guard permits a new full-version publication branch while
preserving existing major/minor publication branches; runtime synthetic tests
execute its actual Bash guards and reject incorrect version/ref combinations.

After integration, require successful exact-main `test` and `windows` jobs,
verify its product archive/checksum/provenance, install that exact artifact and
observe GUI opening/search/restart before triggering the stable publisher.
The publisher independently binds current main, native CI run and product bytes,
refuses existing tags/releases, verifies all four downloadable assets as a draft,
and only then publishes stable. The final run and release provenance are the
publication evidence; the candidate evidence above does not replace them.

No private catalog paths/names, catalog digests, licenses, raw reports or
screenshots are included in this report or release payload.
