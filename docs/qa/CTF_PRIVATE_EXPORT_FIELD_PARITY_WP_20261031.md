# WP — private CTF export parity through independent full-field verification

## Scope / stacking

This bounded WP is based on draft PR #51 at exact parent `e866ba58fdf7ca3786185e1df44b1ebf665294f6`; it does not modify the existing drafts #37–#51 or the main branch. Public source changes are Python offline QA, privacy-safe reporting, tests, workflow and documentation only. Do not merge this child directly into main without reviewing the ancestors.

## Problem closed

Previously `--full-export` only demonstrated that the native CLI could produce CSV/Parquet bytes, not that those exports matched. The existing `tools/verify_exports.py` already independently compared all 15 columns, preserving NULL, Unicode, 64-bit integer sizes and date semantics, but it was not part of the private checkpoint. This WP integrates it into a separate **opt-in** acceptance phase.

## Reproduce locally using only a trusted private workstation

Install `duckdb==1.4.1` into the local Python environment (or a disposable venv). Using an exact, locally verified built `atlas` binary, run the V3 coordinator once for each separate historical source:

~~~sh
python3 tools/qa/offline_ctf_v3.py --cli /absolute/path/to/atlas \
  --expected-sha256 '<source-1-sha256-verified-privately>' \
  --full-export --verify-export-parity --timeout-seconds 1800 \
  --output /private/new-checkpoint-1 /private/source-1.CTF

python3 tools/qa/offline_ctf_v3.py --cli /absolute/path/to/atlas \
  --expected-sha256 '<source-2-sha256-verified-privately>' \
  --full-export --verify-export-parity --timeout-seconds 1800 \
  --output /private/new-checkpoint-2 /private/source-2.CTF

python3 tools/qa/redact_private_ctf_acceptance.py \
  --first /private/new-checkpoint-1 --second /private/new-checkpoint-2 \
  --output /private/new-redacted-summary.json
~~~

Paths and hashes above are placeholders only. The original CTFs remain external read-only sources, with full source digest checks at startup, after import, after byte-exact CTF roundtrip, after each export and on final cleanup. Temporary derived SQLite, exported CTF, CSV and Parquet live in an isolated temporary directory and are removed on normal exit. The full verifier reads **only the derived temporary CSV/Parquet**. It compares every cell in row order, enforces the 15-column schema and 64-bit integer types, and the coordinator additionally checks compared row count equals the import's entry count.

Without `--verify-export-parity`, the redacted result reports `csv_parquet_all_15_fields_equal: NOT_RUN` even when `--full-export` succeeds. With verified equality it is `PASS`; mismatched rows, schema, values, missing DuckDB or an invalid verifier report fail the local checkpoint. A failed attempted verifier is explicitly recorded as `FAIL`, never `NOT_RUN`. The public summary contains only fixed status strings and aggregate counts; the private V3 checkpoint and original CTF must **not** be uploaded.

## CI / tests

Public CI installs pinned DuckDB only for the independent synthetic Python regression on Linux. The synthetic fake CLI emits two generated rows with Unicode, NULL dates/parents and a file size greater than 4 GiB, then generates real CSV/Parquet using DuckDB. Tests verify the positive end-to-end all-column comparison, fail-closed one-cell mismatch, explicit opt-in, source immutability, and redactor allowlist. Existing Linux Rust workspace fmt/tests/Clippy, Windows x64 installer, macOS ARM64/Intel native installer checks remain required.

~~~sh
python3 -m unittest discover -s tools/qa -p 'test_redact_private_ctf_acceptance.py' -v
python3 -m unittest discover -s tools/qa -p 'test_offline_ctf_v3.py' -v
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
~~~

## Release gates

This cannot substitute for original WhereIsIt verification of a **modified** CTF, actual execution on both historical CTFs, native GUI/upgrade QA on Win10/11 and Mac ARM/Intel, Authenticode, Apple notarization or measured target SLO. These are still **NOT_RUN**, and stable 1.0 remains **BLOCKED**. The opaque 0x10 type-1 payload remains undecoded without a source-grounded specification. `v1.0.0-rc.2` is an unsigned prerelease. Never publish private names, source paths/SHA, original CTF/SQLite or raw verifier rows.

## Handoff

At each continuation, verify exact `main`, latest open PRs, current HEAD, four-platform exact-head Actions and installer artifacts. If a job fails, repair it on **this PR branch** with a regression. Otherwise, once original user files and hardware are available, execute this runbook privately; save only the redacted summary in an approved location and keep all private checkpoint material local. Do not claim real-data PASS from synthetic CI or merge unreviewed ancestor drafts.
