# Schema v2 — conditional CTF extension storage (experimental; PR #11)

The SQLite v2 change in this branch is **limited to newly created databases**. It does not yet split `entries.description`, `entries.tags`, full paths, or `search_text`. See [ADR-0002](decisions/ADR-0002-hot-cold-storage.md) for the complete target architecture.

## Compatibility

- New SQLite files initialize at `PRAGMA user_version=2`, with unchanged primary `entries` schema.
- On CTF import, frequently present flags and DOS times go to `ctf_entries`; optional unknown `opaque` binary values are stored by entry ID in `ctf_entry_payload` **only when nonempty**.
- Existing schema v1 databases retain their original dense `ctf_entries(..., opaque BLOB NOT NULL)` layout. The application detects v1 and uses the old import/export SQL; **no on-open migration**.
- Only schema versions 1 and 2 are accepted. Old v0.1 binaries reject schema v2, which is intentional and must be documented before shipping.
- CTF export from v2 LEFT JOINs the optional table; a missing row is reconstructed as an empty byte vector, preserving semantics. The CTF source file is never overwritten.

## What the CI regression taught us

A synthetic CTF created with `opaque=[]` can be parsed back with **nonempty** opaque fields. Consequently, a field being labelled opaque or unknown **does not prove it is sparse**. CI now compares stored payload-row counts to the **actual parsed source records**, not a guessed zero.

This change enables sparse storage when the CTF genuinely has empty opaque extensions, but it is *not* evidence that the 3M-row user's CTF has many such entries or that the total SQLite file will shrink significantly. Measure with the read-only profiler and quantify byte histograms first.

## Release gate

Before treating v2 as production-ready:

1. All Linux, Windows and macOS tests pass and formatted sources satisfy Clippy.
2. A full user-catalog import and exported CTF SHA-256 match the original; no private source data in CI.
3. New/modified CTF is accepted by legacy WhereIsIt on a supported Windows VM, or binary compatibility remains labelled incomplete.
4. Add explicit copy/backup + rollback migration v1 → later v2 hot/cold layout; **no automatic in-place upgrades of a user's 2GB database**.

**No v1→v2 data migration ships in this PR.** The rest of the Hot/Cold work remains in Issue #8 and storage PR #9.
