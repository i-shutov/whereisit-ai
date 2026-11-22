# Windows 1.0 release acceptance — cleaned preflight

On 22 November 2026 the owner approved an unsigned Windows x64 stable release. USB pairing and bound-device rescan are disabled. Ordinary metadata scanning remains supported. Tabs/workspaces, sunburst/archive slices, local analyzer, and macOS/Linux product releases are deferred. Portable code and cross-platform compile/tests remain.

## Observed local 1.0 preflight

These results describe a locally built standard 1.0.0 installer before the hosted source freeze. They do not claim acceptance of an unbuilt final SHA. The release procedure requires another exact-hosted-main installer GUI check before publication.

- 301 Rust tests and strict all-feature Clippy passed. The Python checks passed: 15 RC/stable packaging tests, two release-identity tests and 11 upgrade-contract tests. YAML, inline Python and 42 Bash blocks parsed.
- The setup exited successfully. Installed GUI/CLI hashes matched the staged binaries; CLI reported 1.0.0. The installer was NotSigned, as explicitly approved by the owner.
- Actual interactive Win11 Administrator GUI opened the complete 44-volume catalog, searched and displayed the retained Unicode description, and displayed the generated sidecar note. Disk properties had no pairing or bound-rescan controls.
- GUI created a new SQLite catalog and scanned a generated Unicode folder and two files: one volume, three entries. Native read-only validation returned quick_check=ok and zero foreign-key errors.
- Uninstall removed both product executables and preserved five external file controls plus the new generated SQLite. Reinstall hashes matched. The restarted GUI opened 60 generated volumes and paginated to entries 251–500.
- Four protected original/control files and five external working files were unchanged afterward. No private catalog, path, query, source-file hash, private software license or raw screenshot is included here.

## Retained independent data evidence

Accepted predecessor evidence covers the full 44-volume, 3,046,133-entry supported CTF import; source preservation; a guarded description edit on a derivative and new export independently opened by original WhereIsIt 2014 build 220; reverse vendor edit/import; a byte-identical 144,773,908-byte unchanged roundtrip; independent CSV/Parquet parity for 3,046,133 rows and 15 fields; cancellation, rollback and overwrite refusal.

The 1.0 change does not alter the CTF parser/writer or search semantics. Earlier evidence remains attributed to its actual source IDs rather than relabeled as exact 1.0 evidence.

RC4 main751fa545 passed six classes of 30 warm CLI requests, maximum p95 60.47 ms and 9 MiB peak working set, without concurrent UI automation. A separate concurrent automation series missed the target, maximum p95 237.38 ms; that FAIL is retained. Concurrent-load, cold-cache and GUI-render latency are not certified. The final 1.0 benchmark must run without active GUI automation.

Windows 10 was tested jointly with the owner earlier. Its machine was removed and the owner plans later personal testing. No exact 1.0 Win10, macOS/Linux GUI or physical-USB PASS is fabricated.

## Publication contract

The immutable v1.0.0 release contains only the standard Windows egui setup, ACCEPTANCE.json, provenance.json and SHA256SUMS.txt. The root agent starts publication after exact-main CI and installed GUI acceptance, using dispatch on main or an explicit release/windows-1.0-publish control branch pointing at that validated main SHA.

The publisher checks current main, source/run/repository, required jobs, readiness, approved unsigned scope, privacy and checksums. It creates a draft, downloads the assets and verifies their bytes before publishing stable/latest. Existing RC3/RC4 releases remain unchanged. Final hosted results are also recorded in the release PR and private local report.

The upgrade-fixture regression covers the compact entries view, its unicode_lower trigger, explicit connection closure and visible notes/tags. A generated-only integration with the actual installed 1.0 CLI passed note/tag/search/category validation. No product/runtime change was needed for this QA helper correction.
