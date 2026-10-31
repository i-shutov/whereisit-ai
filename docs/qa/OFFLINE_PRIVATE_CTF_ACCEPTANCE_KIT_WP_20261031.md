# Offline real-CTF acceptance kit — integrated RC3 (2026-10-31)

## Goal

Run the complete integrated native CLI and independent all-field CSV/Parquet
verifier **privately on a trusted Linux x86_64 machine without Internet**.
This closes the packaging dependency that previously prevented DuckDB-based
15-column comparison in isolated execution. It does **not** itself accept
modified CTF compatibility with the original WhereIsIt executable or stable 1.0.

The integration branch is `release/finalization-20261031`, PR #54. The
published artifact is named `WhereIsItAI-offline-acceptance-kit-linux-x64`
and comes from `.github/workflows/finalization.yml`. Check that its Actions
run and artifact are associated with the **exact** commit to be accepted, not
a synthetic PR merge commit.

## What is included and validated in public CI

- Release `atlas` CLI and the native CTF probe, byte-fidelity acceptance,
  RAM-index comparison and synthetic fixture executables.
- Exact-source ZIP, source commit and tree provenance and SHA256SUMS.
- A **pinned DuckDB 1.4.1 wheel for CPython 3.13/Linux x86_64**. CI installs
  it into a new virtual environment with `--no-index --find-links` and runs
  both existing synthetic offline acceptance and redaction regression suites.
- **No private CTF, derived SQLite, exported rows, original paths, notes,
  source hashes or credentials.** Only generated fixtures are used in CI.

The wheel is platform/Python specific. For another platform or interpreter,
use its own separately trusted compatible wheel; never bypass Python ABI
validation or declare an unrun comparison PASS.

## Private verification

Unpack the artifact into a directory separate from every CTF. Verify the
published artifact digest and its internal manifest:

```sh
cd /trusted/offline-kit
sha256sum -c SHA256SUMS.txt
cat SOURCE_COMMIT.txt
python3.13 -m venv /trusted/private-env
/trusted/private-env/bin/python -m pip install \
  --no-index --find-links ./wheels duckdb==1.4.1
/trusted/private-env/bin/python -c 'import duckdb; assert duckdb.__version__ == "1.4.1"'
```

The source CTF is external and read-only. Keep *all* output under a private
location, never inside this public repository. From the extracted `source.zip`
source tree:

```sh
/trusted/private-env/bin/python tools/qa/offline_ctf_v3.py \
  --cli /trusted/offline-kit/bin/atlas \
  --output /trusted/private-reports/NEW-CHECKPOINT \
  --full-export --verify-export-parity --timeout-seconds 1800 \
  /trusted/private-inputs/ORIGINAL.CTF
```

Repeat independently for each of the two historical original inputs. The
script checks the full source generation SHA before/after operations and
before every paged search reuse, validates native SQLite, compares counted
vs fast search page coordinates and exact ordered IDs for two pages, tests
unchanged CTF byte roundtrip, writes ephemeral derived CSV/Parquet, and
requires an independent comparison of all 15 fields and the row count.
Temporary derived data is removed on normal exit. If interrupted, inspect
and securely clean the local scratch directory; the source must remain
untouched. Do not use a timeout/termination as PASS.

For the dual-source public summary, use the strict existing
`tools/qa/redact_private_ctf_acceptance.py` only after both complete
checkpoints exist. Inspect output for accidental personal details before
publishing. The unredacted checkpoint contains private hashes and must
**never** be attached to a PR, issue or Actions job.

## Acceptance boundaries

A PASS for parsed/unchanged CTF, or for generated-only CI, is distinct from
full searchable-description coverage, modified-CTF acceptance in the original
app, manual Win10/11 and macOS Intel/ARM GUI and upgrade testing, independent
target-SLO measurements, Authenticode signing and Apple notarization.
`0x10` is an unknown opaque flag and must never be equated without proof to
the 1,782 undecoded description extensions. Explicitly report all missing
evidence as NOT_RUN/BLOCKED; `1.0.0-rc.3` is an unsigned acceptance candidate.

## Reproduce the CI WP

```sh
python3.13 -m unittest discover -s tools/qa -p 'test_offline_ctf_v3.py' -v
python3.13 -m unittest discover -s tools/qa -p 'test_redact_private_ctf_acceptance.py' -v
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Use the exact HEAD and the status of the new Actions run in the PR handoff.
Do not infer CI success from this document. No unreviewed merge and no
stable release.
