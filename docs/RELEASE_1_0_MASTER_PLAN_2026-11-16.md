> **Новый оперативный срез 2026-11-21:** [Исполняемый Win11/релизный план W0–W7](RELEASE_1_0_WIN11_ACTION_PLAN_2026-11-21.md) актуализирует интеграцию PR #68, draft PR #69/#70 и Windows 11 Pro стенд. Приведённые ниже статусы PR #64/#65 и старого MSRV являются историческими и не отменяют принятого Rust 1.100/egui 0.37. Канонический PR58 UX и все safety/release gates сохраняются; stable NO-GO.

# WhereIsIt AI — мастер-план выпуска 1.0

> **Оперативный срез 2026-11-17:** R2 `PR #62` слит в main (`81f04912`), R3 `PR #63` слит (`405b4ccb`) после полного native CI/installed-upgrade/offline kit PASS. R4 [#64](https://github.com/i-shutov/whereisit-ai/pull/64) и R5 [#65](https://github.com/i-shutov/whereisit-ai/pull/65) — код в отдельных ветках, точная межплатформенная CI ещё не принята. Физическая Windows/macOS GUI и USB-приёмка вынесена в [#66](https://github.com/i-shutov/whereisit-ai/issues/66). **stable_ready=false, 1.0 NO_GO**; не выдавать нативный исходник за финальный релиз.

**Версия плана:** 2026-11-16 / revision 1. **Статус релиза:** NO-GO. **Канонический UX:** [PR #58](https://github.com/i-shutov/whereisit-ai/pull/58), исходный commit a899ac4, безопасная интеграция PR58 в main — 5b421e9. **Источник native USB/поиска:** [PR #57](https://github.com/i-shutov/whereisit-ai/pull/57); [PR #59](https://github.com/i-shutov/whereisit-ai/pull/59) объединён с main в коммите ff87d109a7185e16be593db319e98ba76ba058f7 после успешных exact-head CI, offline QA и installed upgrades. Финальные проверки уже объединённого main — отдельный gate.

Этот документ — оперативный источник последовательности работ и критериев выпуска. Он не объявляет ещё не запущенный Rust/egui готовым, не подменяет реальные испытания синтетическим CI и не меняет опубликованный tag v1.0.0-rc.3. Машиночитаемая начальная матрица: [release/RELEASE_1_0_GATE_MATRIX_20261116.json](../release/RELEASE_1_0_GATE_MATRIX_20261116.json).

## 0. Управляющие решения

1. **В выбранном пользователем PR58 продолжать UX-разработку.** React/TypeScript/Express в apps/ui-prototype — интерактивный прототип и визуально-поведенческий эталон для будущего Rust/egui. Не заменять его более старой компоновкой Win32/SwiftUI PR57, не вводить новый UX без сравнительного аудита.
2. **Финальный продукт 1.0 — offline native Rust/egui** на Windows 10/11 x64 и macOS ARM64/Intel; ни Node, ни Express, ни обязательный браузер не попадают в установщик. Старые Win32/SwiftUI клиенты сохраняются как функциональные регрессионные эталоны до принятой parity.
3. **Существующий Rust/SQLite сервис — источник истины.** Использовать atlas-core, atlas-service, ctf-format, FFI/CLI и закрытые read-only контракты. Не дублировать поисковый движок и CTF-парсер в UI.
4. **Metadata-only by default.** Не считывать содержимое видео, не строить хэши, миниатюры, треки или нейросетевые признаки без явного действия и понятной стоимости. Дубликаты по метаданным — кандидаты, не точные копии.
5. **Безопасность важнее досрочного выпуска.** Сохранение исходных CTF/SQLite, корректная отмена, подтверждённый физический диск, отсутствие удаления из анализа «уже на USB», privacy gate, цифровая подпись — обязательные требования.
6. **Одно завершённое вертикальное изменение на PR.** Иметь один integrator, не плодить десятки микро-PR. После каждого большого блока — commit/push, exact-SHA CI, redacted QA, checkpoint manifest и статус GO/NO-GO.

## 1. Текущий доказанный срез (16.11.2026)

| Область | Наличие кода | Проверка | Блокер/оговорка |
|---|---|---|---|
| Rust ядро/SQLite v3, дерево/поиск/категории | main | Rust/QA/часть real-data проверки | Целевые latency/memory и полный progress/cancel ещё не закрыты |
| Несжатый WhereIsIt Catalog 3.00 | main | 44 диска, 3 046 027 записей, байтовый неизменённый roundtrip, полная 15-полевая CSV/Parquet parity | Оригинальная WhereIsIt не приняла изменённые выходные CTF; исторические 2 CTF недоступны как байты |
| Кандидаты дубликатов/экспорт CSV, Parquet, JSON | main | Контрактные тесты, read-only linked CTF | Нет основания объявлять hash/stream equality по размеру, имени, длительности |
| Native Win32 и SwiftUI | main | Нативные сборки и upgrade CI | Не равно ручной приёмке реального GUI |
| PR57 search-first/physical USB | source merged in main via PR59 | Rust/Win/Mac и offline/upgrade проверки PR59 HEAD прошли | Запуск на реальных 60 USB, UUID-clone и горячее отключение не подтверждены |
| PR58 UX | main | Bun lint/build + safety contract | Это React in-memory demo, а не Rust/egui или реальный CTF backend |
| Rust/egui desktop | R2 native egui preview в main (PR62) | CI Win/macOS ARM/Intel PASS, рабочий каталог+поиск; R3 экспорт PR63 PASS | Основные PR58 UX экраны ещё требуют parity, manual GUI/USB QA; R4 сканирование и R5 QA ZIP не приняты |
| Подписи и нотарификация | отсутствуют | — | RC3 unsigned/ad-hoc; stable block |
| Производительность CTF-поиска | main | p95 209,82–252,83 мс на одном Linux стенде | Цель p95 ≤150 мс пока не достигнута |

**Последний утверждённый публикуемый артефакт:** GitHub Release v1.0.0-rc.3. Выпуски RC1–RC3 не переименовывать и не передвигать их tags; свежий код требует нового RC и новых checksum/provenance.

## 2. Scope contract v1.0

### P0 — обязательно в стабильном 1.0

- Запускаемый офлайн Rust/egui desktop с визуально-поведенческой parity **главных экранов** PR58: компактная верхняя шапка/меню, список 60+ дисков, глобальный поиск, большая виртуализированная таблица, контекст/нижний инспектор, свойства диска, RU/EN, темы и клавиатурное управление.
- Create/Open каталога SQLite v3; просмотр дисков/папок, метаданных и снимков скана; повторный скан с честным прогрессом и атомарным применением; путь к источнику не теряется.
- Поиск AND/OR/NOT, поля/маски/размеры/категории и Unicode, глобальный либо осознанно ограниченный одним диском; результат стабильно возвращается из любых переходов.
- Связанный CTF read-only, Import CTF в отдельный SQLite и безопасный экспорт CTF в допустимых границах без незаметной потери данных; CSV/Parquet/JSON выборок/папок/кандидатов.
- Список физических дисков, независимое OS-обнаружение подключённого устройства, явное связывание с каталожным ID, конфликтные и неизвестные состояния; поиск файла → диск/наклейка → подходящие экземпляры на других носителях.
- Дубликаты по метаданным, с честным указанием основы совпадения; без автоматического удаления; сохранение пользовательских заметок/меток в отдельном sidecar и проверенное резервное копирование/восстановление.
- Работа на 3M+ записей без блокировки интерфейса, защита от stale responses, отмена без потери данных, диагностика и отказоустойчивая миграция; native QA целевых ОС, доверенный установщик и SHA provenance.

### P1 — включить в 1.0 только при отдельном принятом evidence

- Архивные срезы и sunburst: реальный SQLite read-only aggregation backend (без полного переноса 3M строк в UI), понятное ограничение времени/памяти и связь с результатами поиска.
- Сканер видео-заголовков: отдельный **opt-in** плагин, реальный MKV/MP4/AVI/etc parser, границы чтения, контейнеры с удалёнными/неполными заголовками, профилирование. Заявления «+16 B на файл», «+2,5 секунды на 5 TB» в PR58 — гипотезы, не измерения.
- Локальный анализатор: read-only дерево/кандидаты совпадений с USB по метаданным, отчёт CSV для ручной проверки; без операций очистки.
- Группирование видеокопий по сходству: отдельный статус «эвристика»; одинаковая длительность+кодек+разрешение не доказывают одинаковый видеопоток.

Решение по P1 — в ходе UX freeze до RC4: либо отдельная реализованная/проверенная версия, либо элемент интерфейса явно помечен «экспериментальная функция» и выключен в stable. Извлечение рабочих сценариев из PR58 обязательно сохраняется в roadmap; **не маскировать отсутствие backend под 100%-ную feature parity**.

### P2 / после 1.0, если не принято отдельное расширение scope

AI-семантическое распознавание фильмов; миниатюры/декодирование кадров по умолчанию; медиа-парсинг полного файла без opt-in; автоудаление; безусловный «файл уже зарезервирован» по размеру или partial hash; arbitrary compressed/encrypted CTF; облачная синхронизация, плагины выполнения кода. Они не должны случайно стать релизными блокерами P0.

## 3. План по блокам, зависимостям и завершению

### R0. Единый baseline и реестр evidence — 2–4 чел.-дня

**Зависит:** ни от чего. **Участники:** integrator + QA.

1. **Выполнено на PR59 HEAD 9f3eba7:** Linux Rust tests/fmt/Clippy, Windows x64, macOS ARM/Intel, offline kit и установленное RC2→RC3 обновление — PASS; исходный PR59 source слит в main ff87d109. **Осталось:** проверить новый точный main merge SHA и зафиксировать checkpoint R0.
2. Проверить уже выполненное слияние PR59, подтверждая сохранение HANDOFF/checkpoint 2026-11-13, предка PR57 и канонического PR58; закрыть R0 только после повторного main CI.
3. Сопоставить старые 50+ веток: integrated / equivalent / research evidence / obsolete / actual missing feature. **Не** делать массовый merge давно diverged веток поверх SQLite v3.
4. Зафиксировать baseline SHA, версию/lockfile/схему и откреплённые секреты; обновить release gate matrix и one-pager go/no-go.
5. Обеспечить один основной список issues: #60 egui, #7 release, #2 CTF, #3/#8 storage, #4/#5 platform, #6 QA и отдельные блокеры по необходимости.

**Exit:** main содержит PR57+PR58 source (факт слияния выполнен, физическая приёмка всё ещё отсутствует), полный postmerge CI по exact main SHA PASS, исходные CTF и личные каталоги вне публичных артефактов. Пока postmerge CI не завершён, R0 остаётся PARTIAL.

### R1. UX-эталон PR58, сценарии и сервисные контракты — 3–5 чел.-дней

**Зависит:** R0 baseline можно делать параллельно. **Участники:** UX + интегратор.

1. Зафиксировать золотую матрицу экранов: 60+ дисков/все диски; глобальный поиск; выделение файла; свойства/rename F2; сканирование; сравнение; срезы; sunburst; локальный анализатор.
2. Создать screenshot atlas для RU/EN, светлой/тёмной темы, 1024×768, 1366×768, 1600×900, 1920×1080 и HiDPI. В узком окне — компактный fallback с сохранением рабочей таблицы; никакого обязательного горизонтального скролла для поиска.
3. Составить CSV/JSON трассировки: действие → React component → Rust service command → модель SQLite/CTF → ошибка/отмена → native egui widget → QA ID. Всё, что пока только симулируется, отмечать отдельно.
4. Закрепить размеры, плотность строк, видимый контекст, hotkeys, политику фокуса/Back/selected ID, допустимое расстояние курсора и число кликов для 5 ключевых рабочих сценариев. Замерять базовое PR58 и регресс будущего egui; не выдумывать пороги до измерения.
5. Выделить опасные UI-claims (exact duplicates, backup coverage, reclaimed space, physical connected) и отображать уровень достоверности/границы data source.

**Exit:** согласованный reference atlas и UX test contract без перепроектирования; ни один P0-интерактивный экран не останется неописанным.

### R2. Технический фундамент Rust/egui — 5–9 чел.-дней

**Зависит:** R1 контракт, R0 source. **Участники:** egui lead + CI.

1. **Toolchain spike:** Rust в проекте pinned 1.90.0; актуальный eframe 0.37.0 требует rust-version 1.95. Выбрать контролируемый upgrade toolchain (предпочтительно) или конкретную предыдущую совместимую eframe, зафиксировать решение, Cargo.lock, license/notice и проверенные MSRV/CI.
2. Создать apps/desktop-egui как отдельный Rust workspace member с eframe, egui_extras, platform-native file dialogs и выбранным renderer. Сначала smoke «окно запускается и закрывается» на Win10/11, Mac ARM/Intel.
3. Добавить дизайн-систему по PR58: типографика/кириллица, плотность, иконки, меню/панели, светлая/тёмная темы; системная масштабируемость и keyboard-first; AccessKit/screen-reader семантика.
4. Сделать адаптер команд atlas-service с отдельной очередью workers, кэшем только безопасных страниц, request IDs, source-generation pin, cancellable tasks, progress events. UI thread никогда не выполняет CTF SHA и долгие SQLite запросы.
5. Собрать каркас главного экрана, sidebar 60+ дисков, верхний global search, виртуализированную таблицу на 250 строк/страницу, inspector и режимы представления. Данные пока синтетические тестовые, никаких ложных live-USB статусов.
6. Включить golden screenshot/egui_kittest или эквивалентные ручные atlas проверки, npm/React smoke оставить независимым.

**Exit:** настоящий запускаемый native Rust/egui EXE и .app на целевых архитектурах, UI соответствует PR58; Rust tests/fmt/Clippy и native CI зелёные.

### R3. Полное P0 подключение egui к реальному каталогу — 9–15 чел.-дней

**Зависит:** R2 core. **Участники:** egui + service + QA.

1. Open/New/validate SQLite v3 и read-only Link CTF; нативные диалоги и недопустимость overwrite оригинала.
2. Подключить browse/search/advanced Boolean, global scope и explicit disk scope, sort, page, error/retry; state restoration после возврата из копий/папки; сохранение highlighted file ID.
3. Данные таблицы брать только из atlas-service; не строить in-memory копию всех 3M записей. Показ даты/размера/пути/volume ID, nulls/Unicode и значимых оснований для совпадений.
4. Scan Disk: метаданные через существующий backend, progress из реального I/O, snapshots, cancel/transaction rollback, retry/resume policy, запрет обхода системных папок без явного пользователя.
5. Category create/filter/assign, annotations in sidecar с revision-CAS, backup/restore, конфликты после изменения CTF; видимый read-only badge для linked source.
6. Export CSV/Parquet/JSON на полный search/branch/candidate scope (не только видимая страница), no-clobber для linked CTF, Cancel/Progress согласно поддерживаемой backend гарантии.
7. Compact copy из v1/v2→v3 в новый путь, quick_check/foreign_keys/FTS, unknown tables fail closed, rollback и выбор пользователем новой копии.
8. Для PR58 Disk Properties — реализовать storage model: пользовательский label, sticker, notes, фактически известные FS/capacity и source. Не сохранять записанные/вымышленные hardware identifiers в авторитетный CTF.

**Exit:** P0 «найти → определить каталоговый носитель → сравнить кандидатов → экспортировать» выполняется через реальный Rust/SQLite/CTF, включая ошибки, без React и без копирования 3M строк в память.

### R4. Физические USB, видеоориентированные представления — 7–13 чел.-дней

**Зависит:** R3 workflows; часть backend можно готовить после R0. **Участники:** platform-native + egui + media research.

1. Перенести PR57 physical_disks.rs и copy_candidates.rs как backend: Win GUID/volume serial, Mac volumeUUID, user-confirmed bind; shared/clone UUID, system volume, ошибки OS probe и смена буквы диска — fail closed.
2. UI: disk shelf code/real physical mount status (connected, offline, conflict, unverified), Recheck, Pair/Unpair с подтверждением, точные recorded-path/OS-root различия. Не открывать Explorer/Finder без свежей независимой проверки.
3. Экраны PR58: дисковый реестр, свойства носителя, быстрые candidate filters, аналитика объёма и видео; панель copies показывает basis и verified=false пока нет байтовой проверки.
4. Для sunburst/slices приоритетен SQL GROUP BY/read-only summaries с лимитами и отменой, не общая выгрузка 3M записей в рендер. Динамика масштабируется, не размывает исходные данные.
5. Если видео-парсер входит в 1.0: добавить отдельный opt-in plugin adapter, читающий только необходимые диапазоны, тестировать контейнеры, пустые/повреждённые metadata, файлы с moov не в начале; benchmark actual bytes/RSS/DB overhead.
6. Локальный анализатор — только read-only; export кандидатного CSV. Удаление файлов, генерация Shell/PowerShell очистки и автоматика по «already on USB» не допускаются.
7. Группы «разные рипы», «похожие видеопотоки», «совпадает размер/название» — эвристики; даже partial fingerprints не являются безусловной верификацией бит-в-бит.

**Exit:** проверенный источник физического USB состояния и P0 сценарий retrieval; P1 показываются только с реальным backend и раздельной маркировкой неопределённости.

### R5. CTF-совместимость, производительность и fault tolerance — 8–15 чел.-дней

**Зависит:** Rust core уже существует; можно параллельно R2–R4. **Участники:** storage/CTF + independent QA.

1. Golden set: синтетические CTF с Unicode, DOS local dates, пустыми/неизвестными полями, invalid/bounds, category fidelity. Соблюдать указанный supported subset (не весь CTF).
2. В изолированной Windows запустить оригинальную WhereIsIt на **созданных/изменённых** CTF: открыть, просмотреть, сохранить/перечитать, приложить оригинал screenshot/log. До этого native writer не «совместим» только потому, что собственный reader его читает.
3. Два исторических пользовательских CTF: взять байты только при легально разрешённой выдаче поставщиком; если нет — документировать provider blocker, не подделывать execution. Для стабильного writer compatibility нужен формальный scope decision, а не неявный waiver.
4. Не интерпретировать opaque 0x10 как описание без authoritative proof; сохранение байтов отделять от декодирования.
5. На подлинных 3 046 027 строках, плюс синтетических крайних случаях: first page / second page, COUNT vs no-COUNT, Unicode/Boolean 13+ predicates, category 374k, 32 757 duplicate candidates; cold/warm p50/p95, RSS/CPU, dbstat/EXPLAIN, input SHA generation gating.
6. Цель search p95 ≤150 мс на утверждённом эталонном целевом железе. Исторически при full-source SHA на запрос было 209,82–252,83 мс. **Нельзя** убрать SHA для красивого SLO; оптимизировать caching с сохранением security contract или официально изменить SLO до release freeze.
7. База/миграция: измерять размер, WAL/crash recovery, simultaneous reader/writer/checkpoint, атомарность, source integrity, backup/restore и прерывание середины сканирования/импорта.
8. Отдельно померить IO/CPU сканирования 60 USB, не подменять benchmark «за 5 TB» задержками синтетического React-сервера.

**Exit:** проверенная совместимость в оговорённом CTF subset, целевые метрики либо официально утверждённые release-waivers, отсутствие data loss и реальная отмена.

### R6. Физическая нативная QA и UX acceptance — 6–10 чел.-дней

**Зависит:** R3/R4 code complete; R5 performance. **Участники:** независимый Windows/Mac QA + integrator.

Матрица: Win10 Pro x64, Win11 x64, macOS ARM64 и Intel. CI на hosted runner — необходимо, **но недостаточно**. Проверять первые запуски и обновления на чистых/бывших в использовании профилях.

1. GUI: 1024×768/1366×768, 100/125/200% DPI, Retina; RU/EN, длинные Unicode имена, клавиатура, VoiceOver/AccessKit, ошибочные диалоги, focus, selected file, Back.
2. Реальные USB: все отключены, подключение/извлечение, новая буква Windows, sleep/resume, UUID-клон, дублирующие sticker, системный том, частично неуспешный OS probe. При неопределённости никакого green connected.
3. Search/CTF: импорт 3M записей, поиск пока внешние USB отключены, linked source изменён после первой страницы, экспорт и no-clobber, откат отмены, заметки/sidecar.
4. UX сценарии PR58 (замеры до/после egui): поиск фильма, нахождение физического диска, сравнение найденных копий, rename+properties, запуск scan+progress. Считать число кликов, лишние переходы фокуса, путь курсора и видимую информацию; acceptance по screenshot atlas.
5. Негативные сценарии: сломанный CTF, испорченная SQLite, удалённая папка, пропавший USB, запрещённая папка, двойная активация, отмена, закрытие окна во время I/O.
6. Все manual evidence — скриншоты/журналы с обезличенными именами/путями и exact SHA. Отсутствующее реальное устройство отмечать BLOCKED, не PASS.

**Exit:** ни одного открытого P0/P1 дефекта, физические сценарии и экранная parity приняты отдельно по каждой целевой ОС и архитектуре.

### R7. Доверенная упаковка и RC4 — 4–7 чел.-дней

**Зависит:** R6 candidate-ready и R5. **Участники:** release engineer + QA.

1. Windows x64 Installer/portable по согласованному scope; macOS ARM64/Intel PKG и DMG. Сохранять пользовательские SQLite/sidecar при install/update/uninstall, прежние RC не переименовывать.
2. Windows Authenticode signing + timestamp + независимая проверка подписи; Apple Developer ID Application/Installer, Hardened Runtime, notarization через notarytool и stapler с Gatekeeper smoke. Приватные ключи только в защищённых secrets, никогда не в git.
3. 5 платформенных артефактов, все с SHA256SUMS, provenance (точный git SHA, Rust, Cargo.lock hash, OS/runner), LICENSE/THIRD_PARTY_NOTICES, SBOM/security/privacy audit.
4. Изолированные проверенные апгрейды RC3→RC4 и rollback/backup: старая SQLite v3 и sidecar не повреждаются; второй запуск/повторный uninstall идемпотентен.
5. Publish immutable v1.0.0-rc.4 **prerelease**. Не публиковать 1.0.0 stable при неподписанном пакете/незакрытых gate.

**Exit:** RC4 скачивается, проверяется по digest, устанавливается и запускается на каждой OS; подписи проверены отдельно. При отсутствии сертификатов — только test prerelease.

### R8. Независимое решение GO/NO-GO и stable 1.0 — 3–5 чел.-дней

**Зависит:** R0–R7. **Участники:** интегратор + независимый аудитор.

1. Freeze exact source SHA/version, cargo lockfile, схема БД, UI atlas, accept matrix и release notes. Никаких новых функций между freeze и tag.
2. Независимый повтор главного пользовательского пути на чистой машине; regression tests, CTF import/export original-app, privacy, signed upgrade и контроль binary provenance.
3. Обновить release/ACCEPTANCE.json с ссылками на evidence. Переход stable_ready=false→true возможен только после 100% обязательных PASS; отдельно хранить известные ограничения P1/P2.
4. Создать новый immutable tag v1.0.0, собрать из него артефакты и проверить повторную загрузку/checksums; опубликовать stable GitHub Release.
5. Сохранить самодостаточный checkpoint: SHA/manifest, матрица, atlas, результаты ручной QA и отчёт независимого аудита. Подготовить шаблон приёма багов и план 1.0.1.

**Exit:** все release gates пройдены; пользовательские данные не поставляются вместе с бинарниками; официальные downloads и подтверждённый план поддержки доступны.

## 4. Чёткая матрица GO / NO-GO

| Gate | Проверка | Минимум для GO | Исходный статус |
|---|---|---|---|
| G0 | UX authority | PR58 канонический, включён в main | PASS |
| G1 | Native integration | PR57/PR58 в main, postmerge exact SHA CI+offline QA | IN PROGRESS: source merged, main CI pending |
| G2 | Rust/egui | Реальный запуск Windows/Mac, parity P0 PR58 | NOT STARTED |
| G3 | Каталог/поиск/экспорт | Все P0 маршруты через atlas-service; no-clobber/CAS | PARTIAL |
| G4 | Физический USB | Реальные ОС, полная проверка, клоны/смена буквы | BLOCKED |
| G5 | CTF оригинал | Edited/new subset принят оригинальной WhereIsIt | BLOCKED |
| G6 | Данные и производительность | 3M parity, p95 target, memory, progress/cancel | PARTIAL |
| G7 | Безопасность/конфиденциальность | Full source checks, no data leakage, no deletion | PARTIAL |
| G8 | Подписи/установщики | Authenticode, Apple notarization, clean install/upgrade | BLOCKED |
| G9 | Выпуск | Stable tag exact frozen SHA, manifest, auditor sign-off | NOT STARTED |

**Запрет скрытых исключений:** если часть CTF или исторический источник недоступны, можно явно сузить поддерживаемый формат только через зафиксированное продуктовое решение *до* feature freeze; нельзя называть невыполненный тест пройденным. Аналогично с p95 и macOS Intel — отдельное согласование изменения release contract, а не замена expected PASS.

## 5. Дорожки и управление параллельной работой

| Дорожка | Зона ответственности | Старт | Интеграционный checkpoint |
|---|---|---|---|
| A — UX/egui | apps/ui-prototype → apps/desktop-egui; atlas визуальной parity | R1 | Каждый принятый набор экранов |
| B — Service/Storage | atlas-core, atlas-service, быстрый поиск, maintenance/migrations | R0–R1 | Независимый Rust contract+perf |
| C — CTF | ctf-format; оригинальная WhereIsIt; приватные goldens | R0 | Отдельная original-app acceptance |
| D — Windows/Mac physical QA | Native launch, OS volume identity, DPI/USB/accessibility | R2 | Redacted GUI/USB evidence |
| E — Install/release | CI, source provenance, PKG/DMG/Inno, signing/notary | R2 | Exact-SHA package smoke |
| Integrator | Scope, PR review, conflicts, merges, STOP/GO, handoff/manifest | Каждый блок | Один главный актуальный status |

Не редактировать одни и те же Cargo.lock, service API, installer и release/ACCEPTANCE одновременно несколькими агентами. Интегратор принимает завершённые крупные блоки, а не десятки независимых незакрытых PR. Проверяемый ежедневный статус: done / active / blocked / next, exact SHA и ссылки на evidence.

## 6. Обязательная структура checkpoint

В публичной части репозитория — только source, redacted reports, фикстуры, манифесты и документация. Вне публичного репозитория — исходные владельческие CTF, пользовательские SQLite, полные пути, частные source hashes, лицензионные ключи, подписи и неподчищенные логи.

Каждый завершённый R-блок содержит:
- CHECKPOINT.md: дата, scope, exact SHA, branch/PR, тесты, риски и остаток.
- MANIFEST.sha256: контрольные суммы выпущенных публичных/обезличенных артефактов.
- QA_MATRIX.csv / JSON: сценарий, платформа, статус PASS/FAIL/BLOCKED/NOT_RUN, прямая ссылка на evidence.
- Актуальный release gate status и следующий крупный рабочий блок.
- Коммит/push и ссылку на immutable workflow artifact. Исторические материалы предыдущих блоков не перезаписываются.

## 7. Риски первого порядка и способы их снятия

| Риск | Влияние | Что сделать заранее |
|---|---|---|
| Rust 1.90 в workspace vs eframe 0.37 MSRV 1.95 | Блокирует сборку egui | R2 toolchain spike с CI на всех платформах, pin lockfile и сборочного контейнера |
| PR58 demo выдаёт фиктивные USB/видеоданные и метрики | Неправдивый UX/status | R1 traceability; маркировка mock; реальные API только после контракта |
| Scope creep: полный AI/video/TreeSize в 1.0 | Сдвиг релиза | Разделить P0/P1/P2 до R2; новые функции не превращать в P0 без отдельного решения |
| PR57 проверен в CI, но не на реальных носителях | Ложная physical connected идентичность | R6 hardware unplug/replug/clone/system-volume QA |
| CTF writer не проверен оригиналом | Потенциальная несовместимость/потеря пользовательских данных | R5 original-app gate, read-only fallback только при явном scope change |
| p95 150 мс не достигнут при полном SHA | Потеря SLO или снижение безопасности | R5 benchmark/optimization, не отключать проверку исходника ради скорости |
| Нет цифровых подписей | Нельзя выпускать подписанный trusted 1.0 | Запросить защищённый signing path заранее, RC остаётся test-only |
| Неконтролируемое удаление исходников/«дублей» | Необратимая потеря данных | Только read-only review и no-clobber; запрет удаления в 1.0 |
| Два исторических CTF заблокированы provider | Не закрывается полнота исторического формата | Авторизованная выдача байтов либо явно ограниченная спецификация supported subset |
| Недоступна физическая Intel Mac QA | Нельзя заявлять full Intel acceptance | Выделенный Intel Mac стенд или до freeze явно сузить заявленные платформы |

## 8. Оценка трудоёмкости, критический путь и запуск первого блока

Оценка R0–R8: **47–83 инженерных человеко-дня**, без гарантированного календарного срока. Блоки B/C/E могут выполняться параллельно с A. Критический путь — UX контракт → egui/adapter → P0 native parity → physical GUI QA → installer/signatures → final review. При одном инженере календарное время близко к суммарной трудоёмкости, при 3–5 непересекающихся исполнителях уменьшается, но внешние CTF/сертификаты/USB-стенды не распараллеливаются.

**Следующий конкретный ход:**
1. Выполненный source merge PR59 ff87d109 принять к R0 только после повторного полного CI на exact main SHA; записать checkpoint R0, не менять старые tags.
2. Параллельно открыть ветку R1 под UX PR58: atlas + click-travel baseline + трассировка действий к реальному atlas-service; не перерисовывать экран.
3. Создать проверяемый R2 toolchain-spike (MSRV 1.90→1.95 или совместимый egui) и сразу сборочный каркас apps/desktop-egui на Windows/Mac.
4. В отдельной CTF-дорожке подготовить original WhereIsIt edited-CTF тест, Windows машина и протокол, не публиковать исходник.
5. В QA-дорожке оформить физическую матрицу 60 USB и эталонные 3M query/perf fixtures.

**Релизное решение:** источник PR58 принимаем как базу UX. Ни один исходный PR57/PR58/PR59, даже при зелёном CI, сам по себе не разрешает публикацию стабильного WhereIsIt AI 1.0.
