# WhereIsIt AI — дорожная карта до устанавливаемого выпуска 1.0

**Репозиторий:** https://github.com/i-shutov/whereisit-ai (private)  
**Опорный код:** бывший Atlas Catalog 0.1.0; внутренние Rust crate/API names сохраняются до миграции ABI.  
**Статус на 2026-10-28:** v0.1 прототип, проверенный импорт 44 дисков/3 046 027 записей; Mac SDK и новый CTF в оригинале ещё не приняты. Windows тестировался в Wine, не на реальном Windows.  
**Цель:** один устанавливаемый офлайн-продукт для Win 10/11 x64, macOS 13+ arm64/x86_64; компактная навигация по отключённым носителям, сложный поиск и категории, дубликаты фильмов/музыки, CSV/Parquet/CTF.

## Архитектурный принцип

1. **Оперативная БД: SQLite** (Rust `rusqlite`, WAL, индексы, FTS5, версионированная схема). Дерево, метаданные, снапшоты, категории и изменения должны открываться независимо от памяти, наличия исходного диска и плагины отключены по умолчанию.
2. **Аналитика: DuckDB как опциональный модуль после A/B теста**, без включения во все инсталляторы до измерений. Массовые дубликаты и сводки могут выполняться на DuckDB/Parquet или векторизованными Rust-операциями с экспортом результатов обратно в SQLite-кэш.
3. **Polars — developer/экспорт**, а не обязательная пользовательская БД и не зависимость UI. Проверять на тех же данных; не заставлять пользователя ставить Python.
4. **CTF** — внешний формат импорта и экспорта с обязательными проверками обратимости, строгой диагностикой несовместимости и защитой неизвестных полей. Оперативные запросы никогда не используют CTF как хранилище.
5. **Нативный UI:** Win32/Rust и SwiftUI/AppKit поверх общей Rust service/FFI, без Electron или веб-runtime. UI только формирует операции и отображает страницы; не держит миллионы строк.

## Ступени (каждая завершается коммитом, PR, артефактом CI и отчётом QA)

| Блок | Пакет работ | Что сдаём | Критерий приёмки |
|---|---|---|---|
| M0. Bootstrap | Название WhereIsIt AI, GitHub, CI, issues, права, секреты | Исходники, инструкции, воспроизводимый `cargo test` | `main` защищён; исходные пользовательские CTF/SQLite/лицензии не в git; CI стартует на всех ОС |
| M1. Binary compatibility | Несжатый CTF, варианты, категории, группы, Unicode, новые/изменённые записи | `ctf-format`, golden corpus, отчёт original-app import | Идентичный roundtrip неизменённого каталога; новый и изменённый CTF прочитан оригинальной WhereIsIt (на VM) без расхождений |
| M2. Storage/search | Профиль SQLite/DuckDB/Parquet, нормализация путей, индексы категорий, прогресс и отмена импорта, миграция | Таблица замеров, SQL diff, миграция+rollback, bench harness | 3 046 027 записей и те же результаты запросов; `quick_check=ok`, foreign_keys=0; поиск/размер без регресса; CTF roundtrip тот же |
| M3. Desktop UX | Компактная плотная таблица, глобальный поиск, история папок, категория, список совпадений, длинные операции | Две нативные GUI-сборки + скриншоты | В 1366×768 доступен поиск, дерево, таблица; без блокировок; диск виден в глобальных результатах; Win10/11, Mac arm64/Intel smoke |
| M4. Packaging | Inno Setup `.exe`, macOS `.pkg` и `.dmg`; подпись/нотаризация; uninstall; миграция конфигурации | SHA256, установщики, SBOM/лицензии, Release Notes | Чистая VM: install→launch→import/query/export→uninstall; пользовательская БД и исходный CTF не затронуты; проверены подписи |
| M5. End-to-end QA | Реальные 44 диска, 3M записей, дубликаты, CSV/Parquet, Unicode/large files, crash safety | QA matrix+repro, CI artifacts, исправления | No P0/P1 blockers; совпадение запросов Win/Mac; импорт без потери, экспорт без тихих omissions |
| M6. v1.0 | PR review, cherry-pick, tag, release, checksums | Windows setup, Mac pkg/dmg, SHA256, релизный отчёт | Установщики проверены и подписаны, все обязательные acceptance gates зелёные |

## Целевые показатели (SLO как цели, не как текущие измеренные факты)

- На 3M файлов: p95 первой страницы селективного запроса ≤150 ms в прогретом состоянии; непустая широкая категория ≤500 ms после подготовки индексов; UI остается отзывчивым при любой операции.
- Дубликаты фильмов/музыки — первая группа ≤2 s (после инвалидации/приоритетного вычисления), остальные страницы ≤100 ms. Полный анализ 3M — с прогрессом/отменой, без заморозки.
- Размер рабочей БД — измерить в трёх конфигурациях и получить ≥30% экономии относительно 2.16 GB, **только если** сохранены индексы и CTF fidelity; не ставить искусственный лимит менее объёма необходимых метаданных.
- Импорт CTF: пик памяти ограничить заданным бюджетом и обеспечить атомарный rollback; не обещать сроки до независимого замера на том же железе.
- Никаких попыток открывать/хешировать файлы с отключённых носителей без явного действия пользователя. Автоудаление дублей отсутствует.

## Параллельные направления и владельцы (без пересечения редактируемых файлов)

| Дорожка | Рабочий каталог | Задание | Модель при запуске GitHub coding agents |
|---|---|---|---|
| A / `ctf-compat` | `crates/ctf-format`, `docs/CTF_FORMAT.md` | Добить форматы, golden tests, оригинальная WhereIsIt | Codex или Claude, High: неизвестная бинарная семантика |
| B / `storage-bench` | `crates/atlas-core`, `tools/bench_*`, `docs/decisions` | SQLite-vs-DuckDB benchmark и оптимизация схемы, категории | Codex, High: SQL и perf |
| C / `windows-release` | `apps/windows`, `installers/windows`, Windows CI | Compact UI, DPI, Inno Setup, smoke | Codex, Medium/High |
| D / `macos-release` | `apps/macos`, `installers/macos`, Mac CI | SwiftUI, FFI, pkg/dmg, signing/notary gates | Codex/Claude, High |
| E / `qa-ci` | `docs/QA_*`, `tests/`, `.github/workflows` | E2E matrix, security/private data, reproducibility | Codex, Medium |
| MAIN / integrator | `README.md`, `Cargo.toml`, cross-crate service, releases | Contract, review, gate ownership, merge | Main architect / strongest reasoning mode |

### Правило интеграции и чекпойнтов

Каждая дорожка: отдельная ветка `agent/<roadmap-task>`, отдельный PR. PR должен содержать краткое summary, файлы, запуск тестов, метрики до/после, ограничения, SHA-256 артефактов и ссылку на Issue. Integrator не мержит при незавершённой проверке. После каждого Mx: git tag `checkpoint/Mx-YYYYMMDD` либо immutable CI artifact, `CHECKPOINT.md` с commit SHA, `manifest.sha256` и ссылками на основные результаты. Секреты и приватная база остаются вне публичных артефактов.

**Блокирующая зависимость:** M0 source published first. Задачи могут исследоваться параллельно, но агентам нужен доступ к тому же базовому коммиту. M4 может готовить сборочные скрипты раньше M1, но финальный выпуск M6 блокирован M1, M3, M5.

## CI и подпись

- Linux `cargo test/fmt/clippy` + unit parser fuzz/snapshot.
- Windows MSVC build + Inno Setup smoke on Windows runner; Win10 manual/VM test (GitHub runners могут отличаться).
- macOS arm64 `macos-15`, Intel `macos-15-intel`: `build.sh`, `package.sh`, open tests/headless FFI.
- GitHub Actions собирает unsigned **candidate** артефакты. Для публичного подписанного релиза нужны отдельно предоставленные безопасным способом Windows signing certificate и Apple Developer ID Installer/Application + notarization profile. Никаких секретов в issue, чат, git.
- Release gating: SHA256, версия Rust, `Cargo.lock`, platform, tested revision, install/uninstall smoke, версии библиотек и лицензии.

## Принятые ограничения 0.1

- Только исследованный несжатый CTF, а не любая версия оригинальной программы.
- CTF-данные, неподдержанные новым writer, экспортируются только с явным флагом потерь; безопасный экспорт по умолчанию отказывает.
- Дубликаты по имени/размеру — кандидаты, не подтверждение идентичности файлов.
- macOS не тестировалась на SDK до запуска Mac CI; Windows GUI проверен только в Wine. Эти границы нельзя скрывать в релизе.
