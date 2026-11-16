# WhereIsIt AI — интегральный аудит исходников, веток и релиза 1.0

Дата среза: **2026-11-16**. Верифицированные исходные головы: `main=5b421e9d2b779923765459464cd5987c27baaed7`, `UX57=93d7e525dc0433b6790a99250030c9c7fe09ca9a`, `UX58=202970d7206b8e0754e44df48f74a47f1cf2b7fb`. Этот аудит создан одновременно с исходной интеграцией PR #57; итоговый merge в `main` проверяется отдельно. Исторический тег `v1.0.0-rc.3` остаётся неизменным.

## Решение о слиянии веток

- PR #54 и #56 уже объединили стек CTF/annotations/export и приватностный gate в `main`. Не повторять старые cherry-pick.
- PR #58 **слит** как безопасный отдельный **React/Express UI-прототип**, не как работающий Rust/egui. Дополнительно удалены сценарии реального удаления файлов, генерация shell/PowerShell очистки и сетевое прослушивание всех интерфейсов; добавлены отдельные Bun lint/build проверки.
- PR #57 — **реальная** Win32/SwiftUI и Rust реализация USB locator, поиска на всех дисках, навигации по метаданным, а также read-only identity-sidecar. Интеграционное дерево включает 65 изменённых/новых файлов PR #57; конфликтующий `HANDOFF.md` не заменяется старой версией, а дополняется новым разделом.
- Инвентаризация 53 рабочих веток (кроме main) выявила 21 ветку `behind` (нет самостоятельных коммитов после main) и 32 `diverged`. Сразу после PR58 она уже `behind`; PR57 остаётся `diverged` до фактического переноса в main.
- Среди 31 исторической `diverged` ветки — исследования SQLite/DuckDB/RAM, прежние агентские CI/installer варианты, старые CTF-подходы и промежуточные release-кандидаты. `ahead` в Git не значит, что функциональность отсутствует в main: часть изменений была интегрирована через другие PR/merge-коммиты, часть конфликтует с v3. **Массовое автоматическое слияние этих веток запрещено**: требуется feature-by-feature diff, тест и доказательство отсутствия регресса. Исследовательские ветки сохраняются как traceable evidence, не заменяют текущую архитектуру.

## Фактическая функциональная готовность

| Подсистема | Реализовано / подтверждение | Остаток перед stable 1.0 |
|---|---|---|
| Rust, SQLite v3 hot/cold | Каталог, индексы, категории, компакт-копирование с проверками; CI | Реальные нагрузочные SLO, полный GUI progress/cancel и отказоустойчивость на целевых ОС |
| Исходные CTF | На подлинном каталоге: 44 носителя, 3 046 027 записей, неизменённый CTF roundtrip и 15-полевой CSV/Parquet parity | Изменённый/новый CTF в **оригинальной** WhereIsIt, две исторические CTF с запрещённой поставщиком выдачей байтов, смысл opaque 0x10 |
| Search/duplicates/export | Boolean/FTS, source-SHA pinned linked mode, CSV/Parquet/JSON; метаданные дубликатов | p95 150 мс: фактически 209.82–252.83 мс на изолированном Linux; нельзя называть metadata candidates byte-equal |
| Физические USB + search-first | PR #57 содержит Win32/SwiftUI + Rust, snapshot/UUID conflict fail-closed, поиск по 40+ дискам | **Физический** Win10/11 и Mac ARM64/Intel plug/unplug, UUID clone/changed letter, DPI/keyboard/screen reader |
| UI React-эталон | PR #58, 60-дисковый макет, RU/EN, inspector/properties/slices/scan concepts, Bun checks | Это **демо** на фиктивной in-memory БД: нет интеграции с настоящим CTF/SQLite и нет реализованного Rust/egui |
| Целевой egui desktop | Зафиксировано проектное решение ADR-0004 | Создать Rust/egui app и достичь parity по критическим операторам без браузера/Node |
| Видео-плагины/локальный анализ | Только прототип и неподтверждённые оценки 16 B и секунды/5 TB | Реальные парсеры/бенчмарки, собственные feature flags; **не включать в stable как подтверждённый функционал** |
| Сборка/установка | Предыдущие exact-head CI, Windows x64, macOS ARM/Intel; RC3 prerelease с SHA/provenance | Native exact integrated SHA, ручная чистая установка, upgrade/rollback, Win подпись, Apple Developer ID/notary |
| Конфиденциальность | Privacy-gated offline kit и public evidence allowlist | Повторить на финальном exact SHA; не выгружать пользовательский CTF/SQLite и реальные пути |

## Ключевые источники и устаревшие документы

Текущие источники: `README.md`, `HANDOFF.md`, `release/ACCEPTANCE.json`, этот аудит, новый план, `docs/qa/RC3_REAL_DATA_REDACTED_20261031.json`, checkpoint `docs/checkpoint/2026-11-13/`. `docs/PLAN.md` и `docs/STATUS_2026-10-28.md` — **исторические**, не текущий релизный вердикт. Отдельные UX1..UX48 файлы — runbook/кодовые свидетельства, а не подписанная физическая приёмка.

## Последний проверенный CI до объединения

PR #57: native 4/4, offline kit и RC2→RC3 upgrades 3/3 прошли на `93d7e525` (2026-11-03). PR #58: native 4/4 на исходном `a899ac4b`; Bun UI lint/build safety gate прошёл на `202970d7` (2026-11-16). Эти проверки **не** равны successful CI на новом объединённом SHA; свежие workflow обязательны. Ранее `main=1ea5a12f` имел сбой macOS ARM64 smoke-package в отдельном запуске.

## Вердикт

**SOURCE-INTEGRATION CANDIDATE / RELEASE 1.0 NO_GO.** Код допускается к интеграции и проверке при сохранении `stable_ready=false`. Stable 1.0 не может быть объявлен до настоящего egui/parity (если egui входит в scope 1.0), физического GUI QA, работы оригинального CTF, SLO и подписи/нотарификации. Дополнительные UX-сценарии обязаны быть явно помечены `prototype-only`, пока не реализованы в Rust.
