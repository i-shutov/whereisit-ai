# Offline acceptance of a private CTF using the installed-code CLI

**Status:** an executable test harness, not evidence that every historical CTF
variant or changed CTF is accepted in the original WhereIsIt.

This workflow intentionally separates three objects:

1. The original external CTF, opened only for reading and rehashed after work;
2. A temporary SQLite v3 working database, temporary CTF export and optionally
   CSV/Parquet, removed when the program exits normally;
3. A new aggregate-only checkpoint directory, never silently overwritten.

A successful synthetic CI demonstrates that the tooling rejects known
mismatches. **It is not a claim that CI processed a private user catalog.**

## Prerequisites

Use an `atlas` CLI built from the exact source commit under review, with
`--sqlite-version` printing `3.51.3`. Keep the original CTF and its verified
SHA-256 on the local machine. Allow **several gigabytes of free temporary disk
space**, especially for 3M-entry imports, SQLite WAL and CTF export. The
3M-entry import may exceed eight minutes on a fast Linux machine; use the
default 1500-second **per operation** deadline or increase it if necessary.

No Python package other than the standard library is required for the QA
coordinator. The installed application itself uses its native Rust/SQLite
implementation.

On Linux:

```bash
python3 tools/qa/offline_ctf_v3.py \
  --cli /path/to/verified/atlas \
  --expected-sha256 '<SHA-256-of-original-CTF>' \
  --output '/path/to/NEW-local-checkpoint-directory' \
  '/path/to/your-original.CTF'
```

For a full CSV and Parquet test, add `--full-export`; the extra files
remain temporary. On Windows, use the same arguments with
`py -3 tools\qa\offline_ctf_v3.py` and Windows paths.
On macOS, pass the embedded `whereisit-ai-cli` from the matching native
application (or a locally built `atlas` executable).

## Acceptance and privacy requirements

The coordinator requires an actual embedded SQLite 3.51.3; CTF import;
catalog statistics; `validate_catalog`; exact first-page ID agreement
between normal and fast search for fixed Boolean/extension/size queries;
a byte-identical original CTF roundtrip (same SHA-256 and length); and an
unchanged original source SHA-256 after the run. Optional CSV/Parquet exports
measure aggregate byte sizes only. It writes `V3_ACCEPTANCE.json` and
`SHA256SUMS.txt` with SHA-256 and numeric test results. It never deliberately
includes original names, paths or descriptions in reports, and discards
untrusted native error messages.

The output is local to the user's computer. **Do not upload either the input
CTF or exported SQLite/Parquet/CSV to GitHub Actions, issues, PR comments or
other public platforms.** Treat even the source digest as private until
explicitly approved for publication.

A `PASS` does **not** prove that modified CTF output opens in original
WhereIsIt; it only covers unchanged CTF roundtrip and listed query shapes.
Native-only categories and annotations must not be silently dropped.

## Repeatability

Each test uses a new checkpoint directory; run again to a different target.
Check that the input SHA-256 is unchanged. Repeat under warm and cold OS
caches only when measurement methodology explicitly controls caching. The
benchmark's one-off elapsed times are not p95 guarantees.

Review [Issue #23](https://github.com/i-shutov/whereisit-ai/issues/23)
for preserved but currently undecodable CTF descriptions. Byte-exact
roundtrip alone does not mean that all descriptions are searchable.
