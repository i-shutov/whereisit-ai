> **Исторический план. Windows 1.0 stable закрыт 22.11.2026:** [выпуск](https://github.com/i-shutov/whereisit-ai/releases/tag/v1.0.0). Текущий план — [ROADMAP.md](ROADMAP.md): вкладки, sunburst и анализатор PR58 в Windows 1.1; macOS/Linux отдельно. Старые NO_GO, общие платформенные гейты и требования подписи ниже не описывают фактически выпущенную Windows 1.0; действует согласованный scope из [release notes](releases/v1.0.0.md).

# WhereIsIt AI — исполняемый план выпуска 1.0

**Канонический UX:** [PR #58](https://github.com/i-shutov/whereisit-ai/pull/58) (`apps/ui-prototype/`, исходный `a899ac4`); все будущие UX-изменения вести в выбранном пользователем прототипе с visual review, затем портировать в Rust/egui. [Контракт](UX_REFERENCE_CONTRACT_PR58_2026-11-16.md). PR #57 — функциональный/сервисный донор, его старый дизайн не заменяет PR58.

Статус: **NO_GO к stable**, цель: Windows 10/11 x64 и macOS ARM64/Intel, офлайн-работа, единое Rust/SQLite ядро. Принятое направление интерфейса: **настоящий Rust/egui desktop**, не React runtime. Интеграция исходного Win32/SwiftUI PR #57 не означает, что egui уже написан. Версия `1.0.0-rc.3` и её релизный tag не переписываются.

## Контрольные блоки, порядок и критерии

| Блок | Действия | Обязательное доказательство и решение |
|---|---|---|
| R0 — интегрировать/зафиксировать | PR #58 как отдельный mock-эталон; проверить и объединить PR #57, сохранить поздний checkpoint; inventory diverged branches | Один интегрированный commit/PR, полный список изменённых файлов, новых CI runs exact SHA, privacy gate, manifest и статус `source_merged` отдельно от `qa_accepted` |
| R1 — PR58-first UX → egui native vertical | Продолжать PR58 непосредственно в `apps/ui-prototype` (проверка поведения/визуального атласа), затем новый `apps/desktop-egui` с тем же интерфейсом: верхнее компактное меню, реестр USB 60+ слева, глобальный поиск, таблица с пагинацией, инспектор, темы RU/EN и hotkeys; `atlas-service` как единственный источник фактов, не переписывать SQL | Нативный запускаемый EXE/.app без Node/браузера, smoke на Win/Mac, 1366×768 + HiDPI, мышеклики/длина пути курсора, отсутствие лишнего горизонтального скролла |
| R2 — реальная parity вместо mock | Open/New SQLite v3, linked CTF read-only, CTF import/export с гарантией отказа, категорий, duplicates candidates, CSV/Parquet/JSON, дисковая карточка/снимки, нативные диалоги, полноценные busy/cancel/error | Автоматические контрактные тесты идентичности CLI/service/UI, v1-v2→v3 compact copy с rollback и сохранением данных, короткие/длинные запросы, фоновый импорт/экспорт |
| R3 — диск на полке → физический USB | Перенести из PR57 fail-closed physical pairing/OS volume scan, shelf stickers, global search/copy candidate jump, selective browse, invalidation after unplug/replug/sleep, no system root pairing | Независимые реальные Win10/11 и Mac ARM/Intel сценарии: отсутствие USB, смена Windows drive letter, UUID clone/collision, системный том, link source SHA change; видео/файлы **не** читаются по умолчанию |
| R4 — корректность и нагрузка | Оригинальная WhereIsIt читает новый и отредактированный CTF, два ранее предоставленных исторических CTF при разрешённом byte-доступе, opaque 0x10 только с авторитетными данными; latency 3M p50/p95/RSS, progress/cancel и crash/rollback | Согласованное исключение/waiver для недоступных источников только с явным ограничением scope; независимый приватный full-data test; p95 ≤150 ms на выбранном целевом железе **без пропуска SHA-source checks**, либо отдельное документированное изменение SLO |
| R5 — native UX QA и доверенная установка | Матрица Windows x64 Win10/11, macOS ARM+Intel, 100/125/200% DPI, Retina/VoiceOver/клавиатура; чистая установка + rc3→новый RC upgrade/uninstall; подпись Authenticode и Apple Developer ID notarization/staple | Скриншоты/видео и JSON QA с конфигурацией ОС, без приватных путей; подписанные проверенные установщики, SHA256SUMS/provenance; любой P0/P1 => NO_GO |
| R6 — freeze/release | Freeze API/schema, changelog/support boundaries, source SHA/tag/5 установочных файлов, private evidence audit и независимый review | Все mandatory gates PASS, `release/ACCEPTANCE.json: stable_ready=true` только по фактам, `v1.0.0` новый immutable tag; иначе выпускать только отдельный `rc.4+` prerelease |

## Обязательная функциональность v1.0

Компактный офлайн-каталог десятков дисков, поиск файлов по **всем** носителям, навигация по файлу→каталожному диску→подтверждённому физическому USB, безопасный просмотр дубликатов **по метаданным**, SQLite v3, категории, CTF read-only link/import/export с честными ограничениями, CSV/Parquet/JSON, резервное копирование пользовательских заметок, надёжная отмена, критические операции в нативном egui Windows/macOS.

**Не является доказанной функцией 1.0:** AI-разбор видео; продолжительность/разрешение из заголовков; точное сравнение байтов через размер или sparse fingerprint; подтверждение одинаковых видеопотоков по длительности/кодеку; автоудаление файлов; DaisyDisk/TreeSize локальный анализ; импорт произвольных CTF с компрессией/шифрованием. Эти возможности — исследовательские кандидаты `post-1.0`, если не пройдут отдельную физическую и контрактную приёмку.

## Непересекающиеся дорожки

- **A — PR58 UX + egui:** основной UX в `apps/ui-prototype` (никакого редизайна), затем `apps/desktop-egui` с визуально-поведенческой parity; не меняет CTF parser.
- **B — catalog/search:** `crates/atlas-core`, `crates/atlas-service`, latency/rollback; не меняет UI.
- **C — CTF compatibility:** `crates/ctf-format`, original-app golden tests, source fidelity; не правит UI.
- **D — native QA:** `tools/qa`, реальные Win/Mac smoke/accessibility, профилирование; не выдаёт CI за hardware QA.
- **E — delivery:** `installers`, `.github/workflows`, signatures/notarization, upgrade/regression; не публикует stable до GO.
- **Integrатор:** ревью, матрица трассируемости, merge, release/ACCEPTANCE и checkpoint после каждого завершённого крупного блока.

После каждого блока: исходный commit, тесты exact SHA, ссылки CI, `CHECKPOINT.md` + SHA256 manifest, решение PASS/PARTIAL/NO_GO. Публичный git содержит только код, redacted evidence и синтетику. Реальные CTF/SQLite/USB paths остаются локально.

## Stop-the-line правила

1. Никакого прямого удаления пользовательских файлов на основе размеров/имён/sparse-хэшей.
2. Никаких positive OS-connected badges по старой букве диска без OS full scan, pairing и conflict check.
3. Никаких silent-loss CTF export или перезаписи исходников.
4. Никаких релизных обещаний по производительности из синтетического React-сервера.
5. Никакой подмены Rust/egui прототипом React, Win32 или SwiftUI в релизном статусе.
