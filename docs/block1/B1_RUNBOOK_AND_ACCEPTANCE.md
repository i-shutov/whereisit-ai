# Block 1 — real CTF research and execution

**Status: testing harness hardened; real-file tests not executed.** ChatGPT's local Python/container executor currently returns ClientError even on trivial commands. This is an executor failure, not a CTF parser verdict. No actual disk counts, names, private SHA-256, or real timings are claimed until a genuine run is verified.

**Private inputs:** e-books.CTF (4,488,883 bytes), Mp3-60Gb.CTF (2,420,847 bytes). The originals are never written or committed to public GitHub/Actions. Production release v1.0.0-rc.1 is unchanged.

## Tools implemented

- Native Rust **ctf_real_probe**: bounded double-hash source capture, CTF parser, complete counts and distributions (file/folder/archive, hierarchy depth, size buckets, decoded annotations, opaque raw record bytes, coarse attributes), metadata duplicate groups, unchanged parse→write byte comparison with numeric mismatch offset, final source hash verification. Reports no actual file names or paths.
- Native Rust **ctf_real_ram_index**: same-data sorted RAM scan versus extension postings, name trigrams and sparse-description candidates; confirms count and page equality for each fixed query and reports index build time/memory.
- Native Rust **ctf_real_acceptance** (from PR #17): SQLite import, Rust/SQLite query parity, duplicate candidates, CSV/Parquet counts, SQLite→CTF roundtrip, hash-linked cache invalidation, rename reuse, external edit conflict detection.
- Python **run_private_ctf.py**: runs both tools on both sources, writes a local immutable checkpoint: probe.json, acceptance.json, ram_index.json, B1_CHECKPOINT.json, B1_REPORT.md, SHA256SUMS.txt.
- Python **benchmark_columnar.py**: imports real CTF to *temporary* SQLite, exports temporary Parquet, compares native SQLite with DuckDB Parquet scans and Polars lazy/eager. Tested result shape is sorted ID first 250, exact count and SHA-256 of ordered IDs for 14+ predicates; does not disclose actual sample text. Temporary SQLite/Parquet are removed at exit. Warm timing is not a cold-cache/p95 guarantee.

## Windows PowerShell

Build tools in branch research/b1-real-ctf-20261029:

```powershell
cargo +1.90.0 build --release --locked -p atlas-service --example ctf_real_probe --example ctf_real_acceptance --example ctf_real_ram_index
cargo +1.90.0 build --release --locked -p atlas-cli
```

Run from the project directory, changing source paths. The `--assert-real-inputs` flag is an **explicit operator statement**, not automatic verification of the origin or authenticity of a file:

```powershell
$book = 'D:\Catalogs\e-books.CTF'
$mp3 = 'D:\Catalogs\Mp3-60Gb.CTF'
py -3 tools/block1/run_private_ctf.py --assert-real-inputs --probe target\release\examples\ctf_real_probe.exe --acceptance target\release\examples\ctf_real_acceptance.exe --ram-index target\release\examples\ctf_real_ram_index.exe --output "$env:USERPROFILE\Documents\WhereIsIt-B1-001" $book $mp3
uv run --with duckdb==1.5.5 --with polars==1.43.2 python tools/block1/benchmark_columnar.py --assert-real-inputs --cli target\release\atlas.exe --output "$env:USERPROFILE\Documents\WhereIsIt-B1-columnar-001" $book $mp3
```

## macOS / Linux

```bash
cargo +1.90.0 build --release --locked -p atlas-service --example ctf_real_probe --example ctf_real_acceptance --example ctf_real_ram_index
cargo +1.90.0 build --release --locked -p atlas-cli
python3 tools/block1/run_private_ctf.py --assert-real-inputs --probe target/release/examples/ctf_real_probe --acceptance target/release/examples/ctf_real_acceptance --ram-index target/release/examples/ctf_real_ram_index --output "$HOME/WhereIsIt-B1-001" "/path/to/e-books.CTF" "/path/to/Mp3-60Gb.CTF"
uv run --with duckdb==1.5.5 --with polars==1.43.2 python tools/block1/benchmark_columnar.py --assert-real-inputs --cli target/release/atlas --output "$HOME/WhereIsIt-B1-columnar-001" "/path/to/e-books.CTF" "/path/to/Mp3-60Gb.CTF"
```

All output directories must **not already exist**. The native tools can also be downloaded as unsigned, architecture-specific GitHub Actions artifacts. Private source paths and filenames are not emitted in the checkpoint; inputs are identified as `CTF-1`, `CTF-2`. If `--assert-real-inputs` is omitted, the checkpoint records provenance as unverified, even when all synthetic/technical tests pass. The same flag applies to the separate SQLite/DuckDB/Polars experiment; synthetic CI reports retain `unverified_local_ctf` provenance.

### Fail-closed acceptance checks

A successful process exit or `status=parsed` is insufficient. The checkpoint now requires **exact source length and SHA-256** for each input in every phase, the correct dataset order, successful parser diagnostics, byte-identical CTF encoding, matching SQLite import/export bytes, query/count and duplicate/export checks, category-sidecar invalidation tests, RAM indexed page/count parity, and a final unchanged source digest. Missing reports, partial older tool schemas and mismatched/synthetic-only results are marked failed or unverified, never green. These tests do **not** establish that a changed CTF opens in the original WhereIsIt.

The B1 CI verifies a single exact Git commit across platforms, never autoformats or pushes a different commit from a CI run, and does not receive the private CTF inputs.

## Acceptance gates

| Gate | Required evidence | Current state |
|---|---|---|
| Both source SHA-256 | Input hashes before/after and byte sizes | Pending executor |
| CTF format and record coverage | Two real parser reports with disk and field distributions | Pending |
| Native unchanged CTF roundtrip | Byte identity or explicit offset of mismatch | Pending |
| SQLite import and export | Record parity, raw roundtrip or explicit loss-protection error | Pending |
| Search / duplicate parity | Real Rust and SQLite first pages/counts, duplicate groups | Pending |
| Columnar performance | Real SQLite/DuckDB/Polars matching counts/ID digest with measured times and sizes | Pending |
| Independent external edit | Same-hash reuses, altered hash invalidates, no accidental overwrite | Passed on synthetic CI, real pending |
| Original application interoperability | Modified exported CTF opens and works in original WhereIsIt | Not tested |
| GUI / cold-cache / p95 | OS-specific user-facing test results | Not tested |

## CI privacy and checkpoint

The [B1 workflow](https://github.com/i-shutov/whereisit-ai/actions/workflows/b1-real-ctf.yml) builds/smoke-tests Windows, Linux and both Mac architectures on **synthetic-only** data. It never receives original private user CTF files. A green CI is not a real-data acceptance.

After the two real CTF tests run, publish **only aggregate redacted results** and a B1_ACCEPTANCE checkpoint with actual SHA, environment versions, completeness, failures and follow-up parser fixes. Keep original files untouched and do not silently mark blocked gates as passed.
