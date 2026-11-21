> Current status and accepted scope: [release action plan](RELEASE_1_0_WIN11_ACTION_PLAN_2026-11-21.md), [checkpoint](qa/WIN11_RELEASE_CHECKPOINT_20261121.md). Unchecked release-wide items below are not a denial of scoped Win11 PASS.

# WhereIsIt AI — release gate checklist

- [ ] Source `main` has reviewed, passing PRs for M1–M5.
- [ ] Linux, Windows x64, macOS arm64, macOS Intel CI green on exact release SHA.
- [ ] `cargo test --workspace --locked`, fmt and Clippy pass.
- [ ] Real CTF: measured full-catalog counts (current Win11 sample: 44 disks/3,046,133 entries); roundtrip identical for unchanged; original WhereIsIt opens new/edited CTF.
- [ ] No loss for categories, descriptions, DOS times, Unicode and unknown CTF fields, or safe refusal with explicit errors.
- [ ] Search and duplicate counts agree across all platforms; broad categories performance gate met or documented waiver.
- [ ] Windows installer: per-user install / launch / command line / uninstall; user data retained; version shown.
- [ ] Mac pkg/dmg: arm64 and Intel build, launch, CLI/FFI smoke; app identity and icon; safe uninstall instructions.
- [ ] Release signatures and notarization VERIFIED, not assumed. If absent: prerelease/test-only labels and Gatekeeper/SmartScreen caveats.
- [ ] CTF and converted personal SQLite **not uploaded** to repository, PR logs, workflow artifacts or public releases.
- [ ] Artifacts have SHA256SUMS and provenance manifest (OS, runner, Rust, commit, source archive SHA).
- [ ] A clean user profile and a profile with existing config have both been tested; no database loss.
- [ ] Freeze version and changelog; tag `v1.0.0` only after gates pass.

## Дополнительные обязательные gate после UX-консолидации 2026-11-16

- [ ] Новая native сборка Rust/egui собрана и запущена на Windows x64 и Mac ARM64/Intel; React прототип не считается нативным UI.
- [ ] Интегрированные PR #57/#58 проверены на exact `main` SHA; source-only merge не приравнивается к физическому QA.
- [ ] Физические USB identity, OS UUID clone, unplug/replug, смена буквы, system disk deny проверены на реальных Win/Mac.
- [ ] Размер/имя/частичный хэш не объявляются byte-exact backup; никакого удаления/cleanup script по неопределённым совпадениям.
- [ ] Windows/macOS диалоги, клавиатура и доступность, 1366×768, 100/125/200% масштаб подтверждены на реальном GUI.
- [ ] Подписанные и нотарифицированные пакеты и RC3→последний update проверены без потери данных.
- [ ] Все пункты из [исполняемого плана](RELEASE_1_0_EXECUTION_PLAN_2026-11-16.md) имеют ссылку на неприватное evidence и однозначный статус.
