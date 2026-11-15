# Verification — CP-2026-11-13 ingested into local whereisit-ai

Verified locally on 2026-11-15 (UTC). No commits created by this consolidation.

## Git identity

| Ref | Commit | Tree |
|---|---|---|
| Local `main` / checkpoint canonical | `cabdb3d2b139a67729a795ba9c34e4d3144c9c9f` | `00c1e20ab5779475610275e1292cd2c5e333b618` |
| Draft PR #57 `ux/search-first-disk-locator-20261101` | `93d7e525dc0433b6790a99250030c9c7fe09ca9a` | `4f09c2b6175d12899881ad95381ed9d95c92e921` |
| Published tag `v1.0.0-rc.3` (not moved) | `5d91ba12adfa46f5ec6f0310f8e08b590b60f3c6` | (precedes privacy gate on `main`) |

Working tree remained on `main@cabdb3d2`. PR #57 was **not** merged.

## Source zip ↔ git trees

Outer archive SHA for inner source zips matched `MANIFEST.json`:

| Zip | SHA-256 | File path set vs git | Blobs |
|---|---|---|---|
| `WhereIsItAI_main_cabdb3d2_tree_00c1e20a_source.zip` | `0259c7747cc27630e15a00c3a4047524fe7a427dafef4a1fa771f8e1745a38f0` | 422 paths, equal | 422/422 byte-identical to `cabdb3d2` |
| `WhereIsItAI_PR57_draft_93d7e525_source.zip` | `60c555de1e05a4349909ea8e4fb4a5d046ec71625537f6b785548d225e99c687` | 473 paths, equal | 473/473 byte-identical to `93d7e525` |

Note: checkpoint `archive_entries` 580/631 count zip records including directories; tracked **file** counts matching git are 422/473.

Three third-party license paths initially looked mismatched under `git hash-object` on extracted files (`licenses/generic-array-0.14.7/LICENSE`, `licenses/vcpkg-0.2.15/LICENSE-APACHE`, `licenses/vcpkg-0.2.15/LICENSE-MIT`). Re-check with `git cat-file blob` vs raw zip bytes: **equal_raw = true** for all three on both trees. Product sources needed no rewrite from the zips.

`MANIFEST.sha256` over the full extracted checkpoint package: **32/32 PASS**.

## Offline kit

Extracted to `s:\iwork-GH-2\whereisit-ai-offline-kit\` (sibling of the repo, not tracked).

| Check | Result |
|---|---|
| Outer zip SHA-256 | `ba712598b419f98dfc3e7ffb17c268959f15fe1bb6729aecd0d465cc997074b6` PASS |
| Inner `SHA256SUMS.txt` (7 entries) | 7/7 PASS |
| `SOURCE_COMMIT.txt` | `e87e06f3c6b1921bdcce232ee44f0c3028ca9d95` |
| `SOURCE_TREE.txt` | `00c1e20ab5779475610275e1292cd2c5e333b618` (same tree as integrated `main`) |

See [OFFLINE_KIT.md](OFFLINE_KIT.md).

## Ingest into this repository

Copied under `docs/checkpoint/2026-11-13/`: start guide, manifests, `metadata/STATE.json`, evidence index/matrix, integrated reports, next-steps, technical handoff. Did **not** overlay `refs/main/*` or `refs/pr57/*` onto live tree files. Did **not** commit source zips or Linux binaries.
