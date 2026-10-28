# M0 · Import the existing source into GitHub

This repo was created with README+plan first; main currently does not have the 289-file Atlas Catalog source. The full code has been prepared locally in a source ZIP and a Git bundle. **Do not start code agents until source appears here**.

Using source ZIP: clone the private repo, extract the contents of `atlas-catalog/` from the delivered ZIP into the repository root, keep this repo's ROADMAP and ADR docs, verify `git status`, `cargo test --workspace --locked` and `git add/commit/push`.

If using the original Git bundle instead, fetch the bundle as another remote; merge histories with `--allow-unrelated-histories` after verifying the new repo README, and resolve README/docs intentionally, without force push.

Before publishing, inspect `git status --short` and scan for `*.CTF`, `*.sqlite`, private disk paths, license files. GitHub release assets must not include the private converted catalog either.

The local finalization package also introduces installers/windows/InnoSetup, installers/macos/pkg/dmg, new CI and native user-facing rename. Import that *finalization* package if available to avoid losing completed work.