# RC3 privacy release gate: source archive and publisher (2026-10-31)

## Integration provenance

This bounded WP is based on integrated parent PR #54 at exact source commit `c71867bd4ae9bfd7e30e1f469065b2b92af3f1ec`. It is a separate, non-forced branch; no previous draft PR branch or integration commit is rewritten. It complements existing historical public-evidence redaction. It is NOT a new data-import behavior and cannot substitute for real hardware acceptance.

## Closed distribution gap

The native Linux test already invokes `tools/dev/verify_release_identity.py`, which calls `tools/qa/check_public_evidence.py`. However the standalone offline source ZIP creation and the release publisher had no independent last-mile privacy check. This WP adds:

1. **Offline kit:** fail closed in `.github/workflows/finalization.yml` before `git archive`, with the existing allowlisted public-evidence check and five synthetic negative/positive tests.
2. **Release assets:** `tools/prepare_release_assets.py` now invokes the same checker before any installation asset is staged, so the existing RC publisher cannot publish an unredacted source-derived candidate even if upstream jobs were misconfigured.
3. **Regression:** `tools/qa/test_public_evidence_gate.py` uses a temporary *synthetic* project layout only, asserting SHA-shaped lowercase/uppercase tokens and a synthetic private archive path fail without echoing the tokens. Valid synthetic reports pass; malformed JSON fails.

No real personal CTF/SQLite, names, source paths/digests, backups, or credentials are fixtures or artifacts. The checker currently covers a fixed set of historical reports and the private-archive sentinel, NOT an audit of all Git history. Keep this limitation explicit; code SHA-256/build checksums in release manifests are legitimate and not blocked.

## Reproduction / handoff

```sh
python3 tools/qa/check_public_evidence.py
python3 -m unittest discover -s tools/qa -p 'test_public_evidence_gate.py' -v
python3 tools/dev/verify_release_identity.py
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Require exact candidate HEAD native 4/4, installed RC2→RC3 upgrade 3/3, integration regressions and offline kit PASS before merging. GitHub `action_required` (bot-triggered non-execution) is not PASS. The original linked CTF stays authoritative read-only; source generation is cryptographically checked at every repeat use. User annotations and derivative cache are independent; uncertain matches show conflicts; opaque flag `0x10` remains uninterpreted without source-grounded evidence.

RC3 is an unsigned **prerelease** only. Stable 1.0 remains BLOCKED by original WhereIsIt verification of modified CTF, two restricted historical sources, physical Windows 10/11 and Mac Intel/ARM GUI/accessibility, Authenticode and Apple notarization, and target real-data SLO (150 ms p95 not met). No merges/releases are authorized by this child PR until full exact-HEAD CI verification.
