# WP — two local private CTF checkpoints → redacted summary and physical QA gates

**Scope:** This changes QA tooling only, stacked on draft #50 at b5f9f3e758020cf993c5b04002d683804c268289. No CTF parser, cache schema, native app, or user catalog changes. Keep ancestor drafts untouched. Public CI uses synthetic inputs only.

## Private local execution

Build or install the exact-source Atlas CLI and locally verify its binary identity. Run the existing offline coordinator separately for **both** historical CTF files, with fresh output directories. The original external files are read-only, rehashed before/after, and temporary SQLite/CTF exports are deleted on normal process completion. Placeholder paths below are not real user paths:

~~~bash
python3 tools/qa/offline_ctf_v3.py --cli /path/to/atlas \
  --expected-sha256 '<verified-private-source-1-sha256>' \
  --output /private/new-checkpoint-1 /private/first.CTF
python3 tools/qa/offline_ctf_v3.py --cli /path/to/atlas \
  --expected-sha256 '<verified-private-source-2-sha256>' \
  --output /private/new-checkpoint-2 /private/second.CTF
python3 tools/qa/redact_private_ctf_acceptance.py \
  --first /private/new-checkpoint-1 --second /private/new-checkpoint-2 \
  --output /private/NEW-redacted-summary.json
~~~

On Windows use py -3 instead of python3. `--full-export` alone checks successful CSV/Parquet creation but **does not** compare fields. To also run the independent local DuckDB verifier on all exported rows and all 15 columns, install `duckdb==1.4.1` in the local Python environment and pass **both** `--full-export --verify-export-parity` to each `offline_ctf_v3.py` invocation. The runner fails closed on a field mismatch, a row-count mismatch with the imported entry total, a missing DuckDB dependency or a verifier crash. The original CTF is SHA-rechecked before reuse, after roundtrip and after every temporary export. The verifier's raw stdout/stderr and file locations never appear in the checkpoint; only fixed status, row count, column count and checker ID are included. Use `--timeout-seconds` for the actual target machine if necessary.

Each original V3_ACCEPTANCE.json is **private**: it contains source/output/CLI SHA-256, numeric diagnostics and hashes of result IDs. Never upload it to GitHub, public issues, PR comments, CI, or release artifacts. The redactor copies no source paths, file names, SHA hashes, IDs, query text, raw errors, native logs or source bytes. It emits static labels CTF-1 and CTF-2, numeric aggregate counts and a closed PASS/FAIL/NOT_RUN vocabulary. It rejects duplicate source hashes without disclosing them; a missing/mismatched CLI digest or any failed mandatory phase cannot produce overall PASS. The redacted `csv_parquet_all_15_fields_equal` check is `NOT_RUN` unless the optional independent verifier produced explicit evidence, `PASS` only when all fields and expected rows agree, and `FAIL` for invalid/failed evidence. `NOT_RUN` never constitutes acceptance of export parity. A newly created output is required; existing output is never overwritten. It returns exit 0 when both **available local** checks pass, 1 when a safe FAIL report was produced, 2 for invalid/unsafe input or destination.

**Important:** available-local-checks PASS is only a technical result. The source provenance is not independently attested, CSV/Parquet field equality and original-app modified CTF interoperability are unverified, and stable 1.0 is always BLOCKED. The summary explicitly records all physical/signing/performance gates as NOT_RUN. Never interpret opaque flag 0x10 as missing descriptions without grounded original-app specification.

## Physical acceptance matrix (human test required)

Record PASS, FAIL or NOT_RUN with candidate source commit, installer digest, local evidence ID, date and platform. **Do not put private CTF names, paths, source hashes, original screenshots containing filenames, or secrets in public QA artifacts.**

| Platform / gate | Required real test | Current evidence |
|---|---|---|
| Windows 10 x64 | Clean install, compact 1366×768, DPI/keyboard; linked CTF disk tree, Boolean search/pagination, duplicate candidates, new-file CSV/Parquet, annotation confirmation/conflicts, safe uninstall | NOT_RUN |
| Windows 11 x64 | Same vertical scenario, predecessor-to-candidate in-place upgrade/rollback, retain external catalog and native notes | NOT_RUN |
| macOS ARM64 | Native app/FFI, Gatekeeper, linked CTF, search/export, annotation activation/revoke, PKG/DMG upgrade/rollback | NOT_RUN |
| macOS Intel | Same scenario on x86_64, verify native executable architecture and application identity | NOT_RUN |
| Original WhereIsIt | Modify **an isolated working copy** of CTF in controlled ways (rename/add/description/category), open/search in the original Windows application and verify unknown-byte preservation | NOT_RUN |
| Full-data performance | On target hardware, collect 3M-entry cold/warm distributions, p50/p95, RSS, disk, sync/export and duplicate candidate latencies; compare against explicitly adopted SLO | NOT_RUN |
| Trust and signatures | Authenticode chain on Windows; Apple Developer ID signature, notarization ticket and Gatekeeper on both Mac architectures | NOT_RUN |

On external source modification, pinned reads must return source_changed and require explicit re-sync; ambiguous mappings remain conflicts, never auto-applied. Cancellation, crash, failed migration/upgrade and uninstall must preserve the external CTF and independent annotation SQLite. A mismatch is a FAIL, not a rounded estimate or fictional PASS. The installed unsigned rc.2 candidate is not a stable 1.0 release.

## Reproduce this bounded WP without private data

~~~bash
python3 -m py_compile tools/qa/redact_private_ctf_acceptance.py tools/qa/test_redact_private_ctf_acceptance.py
python3 -m unittest discover -s tools/qa -p 'test_redact_private_ctf_acceptance.py' -v
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
~~~

Synthetic tests deliberately inject private-looking source strings, hashes, paths and error codes, verify only allowlisted aggregate output, two distinct sources, matched CLI, source/roundtrip identity, missing checks, invalid JSON and no-clobber. They do not run a physical GUI or decode any real CTF.

**Handoff:** Do not merge this draft or its ancestors until reviewed. The next WP should perform actual private execution and manual GUI QA when original CTF files, original WhereIsIt, target hardware and signing credentials are available. Issues #2–#8, #19 and #23 remain blocked.