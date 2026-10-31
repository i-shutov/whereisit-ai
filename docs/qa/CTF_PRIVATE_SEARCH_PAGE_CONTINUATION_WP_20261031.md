# WP — bounded search pagination and continuation parity on local CTF checkpoints

## Scope / architecture

Stacked on draft PR #52 at exact parent `a6614af6b3f61506f88bbc8f4f1412df89e50d73`. No ancestor branch, main, private CTF or release is changed. The private offline V3 runner previously compared **only the first 100 IDs** from `search_fast` and counted `search`; an incorrect fast continuation flag, replayed page offset or incorrect second page could pass unnoticed.

The native `search_fast` API **does not return `total`**. It returns `has_more`, `offset`, `limit` and `items`. The counted `search` returns `total` instead of `has_more`. This WP validates each side against its actual schema rather than assuming a fast total exists.

## Implemented fail-closed checks

For each of four fixed native query templates, check the first page (limit=100) and, **only when counted total exceeds 100**, the second page at offset=100. For both sampled pages:

1. Check exact integer `offset`/`limit` coordinates from both APIs (no replay/mislabel).
2. Check ordered entry-ID sequence equality, strict total and page size, and count stability between samples.
3. Check that `search_fast.has_more` is an actual boolean equal to `offset + page_length < counted_total`.
4. SHA-256 rehash the authoritative external CTF before each native search call, including the second page, to fail closed on a generation change.
5. Only store aggregate counts/timings and private first-page ID digest in the local V3 checkpoint. No names, paths or raw search rows in the public report.

The source CTF remains **read-only**; derived SQLite and exports remain temporary. Native user annotations stay separate from the derived cache. The public redactor now requires explicit `bounded_search_page_continuation` evidence for each of the four query templates to return local-available-checks PASS. Old first-page-only checkpoints no longer qualify; rerun privately instead of upgrading historical results by assertion.

**Limit:** these checks sample at most the first two pages. They do **not** prove equality over all ~3 million entries, cold-cache performance SLO, or original WhereIsIt compatibility. Do not describe them as exhaustive search parity. Deep pagination and real-data acceptance remain separate gates.

## Reproduce on synthetic data

```sh
python3 -m unittest discover -s tools/qa -p 'test_redact_private_ctf_acceptance.py' -v
python3 -m venv .ctf-qa-venv
.ctf-qa-venv/bin/python -m pip install --disable-pip-version-check duckdb==1.4.1
.ctf-qa-venv/bin/python -m unittest discover -s tools/qa -p 'test_offline_ctf_v3.py' -v
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Synthetic cases include 205-entry second-page consistency, second-page ID mismatch, deliberately inverted `has_more`, altered page offset, and rejection of a fabricated/legacy first-page-only public checkpoint. Public CI uses synthetic-only inputs and must remain green on Linux, Windows x64, macOS ARM64 and Intel; no tests are disabled.

## Private original checks and handoff

When real CTFs are available, use `tools/qa/offline_ctf_v3.py --full-export --verify-export-parity` separately on each locally supplied historical CTF, compare the checkpoint via `redact_private_ctf_acceptance.py`, and retain originals and detailed reports **only locally**. Never copy CTF, source SHA, derived SQLite, file paths, user names, notes or raw rows into public GitHub/Actions, including comments and artifacts.

Validate the exact branch HEAD and Actions matrix before continuing. This draft must not merge directly into `main` without reviewing the stacked ancestors #37–#52. Remain **BLOCKED** for actual two-CTF runs (#19), modified CTF opened in original WhereIsIt (#2), Win10/11 and Mac Intel/ARM physical GUI and upgrade, Authenticode/Apple notarization, and measured target SLO (#3, #6–#8). Opaque flag `0x10` has no source-grounded meaning; do not infer that it explains 1,782 descriptions (#23). `v1.0.0-rc.2` is an unsigned prerelease, not stable 1.0.
