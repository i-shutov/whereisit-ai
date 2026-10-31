# GitHub: первичная публикация и сопровождение

## Репозиторий

Приватный репозиторий `i-shutov/whereisit-ai` уже создан. Его GitHub-содержимое и локальный bundle должны быть синхронизированы до назначения coding agents на задачу с кодом.

## Если у вас Git bundle из комплекта передачи

```sh
git clone WhereIsItAI-0.1.0-git.bundle whereisit-ai
cd whereisit-ai
git remote rename origin source-bundle
git remote add origin https://github.com/i-shutov/whereisit-ai.git
# Только если GitHub main всё ещё пустой:
git push -u origin main
git push origin --tags
# Если в GitHub main уже есть README/план, не используйте force push:
# git fetch origin main && git merge --allow-unrelated-histories origin/main
# git push -u origin main
```

При публикации через подключённый GitHub API вместо Git bundle можно использовать тот же исходный пакет; не смешивайте два способа без сравнения истории.

## CI и артефакты

`.github/workflows/build.yml` выполняет Rust-тесты, Clippy/fmt, сборку Windows и macOS и прикладывает ZIP к успешным workflow runs. Первый прогон требуется реально наблюдать; наличие YAML **не** подтверждает успешную сборку на GitHub. Mac runners и Windows runners могут иметь разные платформенные ограничения.

Windows ZIP `AtlasCatalog-0.1.0-Windows-x64.zip` предлагается хранить как prerelease asset, а не коммитить. Приватная пользовательская база (`private-catalog-AtlasCatalog-v0.1.sqlite.zip`, около 420 MB) и исходный CTF **не должны** попадать в GitHub даже как release asset. `.gitignore` не гарантирует защиту, если файлы добавляются с `-f`.

## Первые задачи (issues)

1. `P0 / CTF`: проверить новый/изменённый CTF на оригинальной WhereIsIt, сформировать golden набор.
2. `P0 / Native`: собрать и провести smoke test на Windows 10/11 и Apple Silicon Mac.
3. `P1 / Performance`: убрать дублирование длинных путей и уменьшить размер БД при сохранении поисковых характеристик.
4. `P1 / Categories`: индексировать широкие наследуемые категории (контроль 374 259 записей), обеспечить корректную инвалидацию.
5. `P1 / Import`: потоковый CTF, отмена, прогресс и rollback.
6. `P1 / Compatibility`: mapping CTF categories/flags/group hierarchy, без молчаливой потери данных.
7. `P2 / Duplicates`: опциональный SHA-256 доступных файлов отдельно от сканирования метаданных.

## Ветки и процесс

- `main`: интеграция только после проверок.
- `feat/ctf-*`: импорт/экспорт, золотые CTF-тесты.
- `feat/search-*`: запросы, категории, индексы, профилирование.
- `feat/windows-*`, `feat/macos-*`: отдельные native-клиенты.
- В каждом PR: сценарии QA, замеры регрессий, изменение размера БД, `cargo test`, формат/Clippy, влияние на CTF roundtrip.

Подробнее см. [`STATUS_2026-10-28.md`](STATUS_2026-10-28.md) и [`PLAN.md`](PLAN.md).

Дорожная карта: [ROADMAP_TO_1.0.md](ROADMAP_TO_1.0.md), список готовности: [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md).
