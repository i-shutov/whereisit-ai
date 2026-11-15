# Offline kit (local companion, not in git)

Privacy-gated Linux acceptance kit from checkpoint CP-2026-11-13.
Binaries and the DuckDB wheel stay **outside** this repository.

## Local path

`s:\iwork-GH-2\whereisit-ai-offline-kit\`

Sibling of the git working copy. Do not copy `bin/`, `wheels/`, or `source.zip` into the public tree.

## Expected digests

| Artifact | SHA-256 |
|---|---|
| Outer zip `WhereIsItAI_RC3_privacy_gated_offline_kit_13029884531.zip` | `ba712598b419f98dfc3e7ffb17c268959f15fe1bb6729aecd0d465cc997074b6` |
| Inner `SOURCE_COMMIT.txt` | `e87e06f3c6b1921bdcce232ee44f0c3028ca9d95` |
| Inner `SOURCE_TREE.txt` | `00c1e20ab5779475610275e1292cd2c5e333b618` (identical to integrated `main` tree) |

Inner `SHA256SUMS.txt` covers `source.zip`, `bin/atlas`, QA probes, and the CPython 3.13 manylinux DuckDB wheel. Verify with that file after extraction.

## Provenance

```
channel=unsigned-integration-candidate
real_user_data_included=false
stable_1_0_accepted=false
run_id=41169239686
```

This kit is **not** a signed Windows/macOS installer. Official RC3 installers remain on the GitHub release tag `v1.0.0-rc.3` (commit `5d91ba12…`, which **precedes** the later privacy-gate merge on `main`).

## Privacy

Do not place private CTF, owner SQLite, personal paths, private content digests, or signing secrets into this kit directory, into git, or into public CI artifacts.
