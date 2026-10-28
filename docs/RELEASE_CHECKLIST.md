# WhereIsIt AI — release gate checklist

- [ ] Source `main` has reviewed, passing PRs for M1–M5.
- [ ] Linux, Windows x64, macOS arm64, macOS Intel CI green on exact release SHA.
- [ ] `cargo test --workspace --locked`, fmt and Clippy pass.
- [ ] Real CTF: 44 disks/3,046,027 entries; roundtrip identical for unchanged; original WhereIsIt opens new/edited CTF.
- [ ] No loss for categories, descriptions, DOS times, Unicode and unknown CTF fields, or safe refusal with explicit errors.
- [ ] Search and duplicate counts agree across all platforms; broad categories performance gate met or documented waiver.
- [ ] Windows installer: per-user install / launch / command line / uninstall; user data retained; version shown.
- [ ] Mac pkg/dmg: arm64 and Intel build, launch, CLI/FFI smoke; app identity and icon; safe uninstall instructions.
- [ ] Release signatures and notarization VERIFIED, not assumed. If absent: prerelease/test-only labels and Gatekeeper/SmartScreen caveats.
- [ ] CTF and converted personal SQLite **not uploaded** to repository, PR logs, workflow artifacts or public releases.
- [ ] Artifacts have SHA256SUMS and provenance manifest (OS, runner, Rust, commit, source archive SHA).
- [ ] A clean user profile and a profile with existing config have both been tested; no database loss.
- [ ] Freeze version and changelog; tag `v1.0.0` only after gates pass.
