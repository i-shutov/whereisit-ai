# WhereIsIt AI — план выпуска 1.0 с Windows 11 Pro QA

**Оперативный срез:** 2026-11-21. **База:** `main`, SHA `ddd9215b0926c5e74482a3cbf7f10c229a66fb88`. **Решение:** `stable_ready=false`, stable 1.0 — **NO-GO**. Это дополнение к [мастер-плану 16 ноября](RELEASE_1_0_MASTER_PLAN_2026-11-16.md), [исполняемому плану](RELEASE_1_0_EXECUTION_PLAN_2026-11-16.md), [PR58 UX contract](UX_REFERENCE_CONTRACT_PR58_2026-11-16.md) и [машинным воротам](../release/RELEASE_1_0_GATE_MATRIX_20261116.json). При расхождении исторических статусов ориентируйтесь на этот датированный срез и **заново проверенные GitHub SHA**; формальный release scope и политика безопасности старых документов сохраняются.

> **Privacy:** IP/учётные данные нового Windows 11 VPS, RDP, приватные пути, CTF и реальные имена файлов не сохранять в открытом GitHub. Внешний адрес передан владельцем только для локальной настройки. GitHub-hosted CI и self-hosted runner — разные доверенные контуры.

**Owner directive 2026-11-21:** The owner runs Win11 RDP, PowerShell, egui GUI QA and Codex **as Administrator**. Do not require or insist on a separate non-admin account. Interactive Administrator and the privileged self-hosted runner service are separate execution contexts: avoid automatically executing unreviewed public-PR code through the latter.

## 1. Зафиксированные факты и незакрытые проверки

| Область | Доказанный статус | Чего НЕ следует утверждать |
|---|---|---|
| `main`: PR [#68](https://github.com/i-shutov/whereisit-ai/pull/68) | Слит; postmerge [native CI #459](https://github.com/i-shutov/whereisit-ai/actions/runs/44321217977) Linux/Windows/macOS ARM/Intel — PASS. [RC2→RC3 upgrade #250](https://github.com/i-shutov/whereisit-ai/actions/runs/44321217699) завершён PASS после отдельного повторения отменённого macOS ARM job. | Не означает stable или полную GUI-приёмку. |
| Windows 10 Pro QA | Hosted egui bundle, 60 синтетических дисков, SQLite/search/exports/no-clobber, NTFS-прогресс и Esc rollback с неизменённым каталогом — проверены в [PR #68](https://github.com/i-shutov/whereisit-ai/pull/68). | Нет полной PR58 parity, физического USB или Win11 evidence. |
| [PR #69](https://github.com/i-shutov/whereisit-ai/pull/69) | Draft `qa/pr58-egui-parity-win10-20261118`. Улучшения Back/selection/scroll, other-disk candidates, F12 renderer screenshots (opt-in), локализации, тем, диалогов, парный атлас; подтверждения — в `docs/qa/PR58_EGUI_*` ветки. | Не в main; categories/analytics/local analyzer и систематический DPI/мышь-аудит остаются частично/полностью открыты. |
| Windows 11 Pro VPS | Новая ОС развернута владельцем. Предыдущий локальный preflight Windows 11 Pro build 22631 выявил запущенные службы трёх независимых репозиторных runners (включая `win11-whereisit`) и установленное dev-окружение. | GitHub Online/Idle и успешный **runner smoke на этом хосте** не доказаны; доступность интерактивного RDP/GUI в текущем цикле нужно проверить отдельно. |
| [PR #70](https://github.com/i-shutov/whereisit-ai/pull/70) | Draft `ci/win11-vps-smoke-20261120` — ручной workflow только на доверенном `main`, без checkout, без внешнего кода и без secrets; точный label `win11-whereisit`. | Пока PR не слит, это **не выполненный** smoke. |
| Данные и CTF | Существуют исторические read-only проверки 44 дисков и 3 046 027 строк, CSV/Parquet fidelity. | Изменённый CTF **не принят** независимой оригинальной WhereIsIt; две исторические CTF остаются недоступными как байты провайдера. |
| Сборки | RC3 — зафиксированный prerelease; QA preview bundles — unsigned/ad-hoc. | Подписанного/нотарифицированного stable 1.0 нет. |

## 2. Назначение трёх контуров проверки

**Контур A — GitHub-hosted CI (авторитет кода и артефакта).** Запускает Rust workspace/fmt/strict Clippy, privacy, offline QA, Windows/macOS ARM/Intel build и upgrade tests на **точном SHA**. Produces installer/preview ZIP, SHA-256/provenance. Успешный CI не подменяет запуск на физической ОС.

**Контур B — Windows 11 self-hosted runner (сначала только подтверждение стенда).** Запускать **исключительно** просмотренный, вручную инициированный workflow из доверенного `main` (PR #70). Проверить Online/Idle, точную метку, OS x64/build, identity и видимость toolchain; факт PASS оформить отдельным checkpoint. **Не** запускать на администраторском сервисе `pull_request`, `pull_request_target`, чужие PR, произвольный checkout, чужие скрипты или workflow с секретами. Запрещено выполнять сборку untrusted кода на привилегированном runner; если нужны регулярные build jobs — проектировать отдельную непривилегированную изолированную среду.

**Контур C — интерактивная Windows 11 Pro RDP QA (авторитет GUI).** Codex/QA в разблокированной пользовательской сессии независимо от runner service запускает проверенный native egui EXE и synthetic NTFS, снимает рендерные скриншоты и протоколирует действия. Сервис runner в Session 0 не является эквивалентом интерактивного UI. Не вмешиваться в соседние проекты/службы, службы двух остальных runners, firewall, SSH, VPN, драйверы или системное администрирование.

**Персональные данные:** только генерируемые SQLite/файлы в отдельном каталоге WhereIsIt; личные CTF и разрешённый реальный большой каталог — отдельно, **read-only**, без загрузки байтов, файловых путей, хэшей частного CTF или скриншотов в публичный GitHub.

## 3. Критический путь и крупные блоки

### W0 — интеграция baseline (сначала, один integrator)

1. Подтвердить `main ddd9215b` и актуальные ветки; не повторять работу PR #68.
2. В [PR #69](https://github.com/i-shutov/whereisit-ai/pull/69) отделить **проверенные исправления GUI** от ещё не принятой полной UX parity. Проверить свежий SHA, hosted CI, notice/lock/license coherence, недопустимость QA-screenshot features в штатном бинарнике; решить, можно ли принять исправления и перенести незакрытые вопросы в отдельную матрицу. Merge только без потери доказанных функций PR #67/#68; затем **новый postmerge CI на main**.
3. Просмотреть security-концепцию [PR #70](https://github.com/i-shutov/whereisit-ai/pull/70), исключить выполнение untrusted PR на admin runner; merge можно делать отдельно, после ревью. Не добавлять хост IP в репозиторий.
4. Обновить `HANDOFF`/release checkpoint только фактами, а `release/ACCEPTANCE.json` оставить `stable_ready=false`.

**Выход W0:** один понятный baseline SHA, проверенная интеграция PR69 (или документированное основание для продолжения draft), отдельно готовый trusted manual runner workflow.

### W1 — приёмка Win11 host/runner (новый блок)

1. В активной RDP-сессии подтвердить Windows 11 Pro x64, реальный build, свободное место, OS/GPU/renderer, RAM, DPI, журнал безопасности, текущую учётную запись и отсутствие запуска из service session. Не менять систему без необходимости.
2. Проверить все три службы локально **read-only**, но для WhereIsIt подтвердить отдельно GitHub **Online/Idle** и отсутствие конфликтов с двумя другими runners; не требовать повторной регистрации, если runner уже есть.
3. После ревью и слияния PR70 один раз вручную вызвать `Windows 11 VPS runner smoke` **из main**, зафиксировать run ID, SHA, `RUNNER_SMOKE_PASS`, наличие Rust 1.100/MSVC/Git/Python; отсутствие какого-то утилитарного инструмента записать, не ломая хост.
4. Проверить доступ к интерактивному GUI в **другой, обычной RDP-сессии**; при UAC/служебной блокировке не утверждать, что runner решил GUI-блокер.

**Выход W1:** `WIN11_RUNNER=PASS/BLOCKED` и `WIN11_INTERACTIVE_SESSION=PASS/BLOCKED` с отдельными доказательствами. До этого `WIN11_QA=NOT_RUN`.

### W2 — native Win11 egui QA на реальном NTFS (новый P0)

1. Скачать QA bundle из успешного hosted workflow **ровно для тестируемого SHA** (предпочтительно принятого main после W0). Проверить digest outer Actions archive, SHA inner ZIP, provenance, manifest, SHA GUI/CLI. Не смешивать SHA.
2. В изолированном профиле выполнить существующий PowerShell 5.1 harness (6 headless + 60 GUI synthetic volumes) и реальные Windows 11 GUI сценарии: Open/New SQLite, поиск глобальный/по диску, Boolean/Unicode, >250 строк/пагинация, inspector/Back, labels/properties/F2, CSV/JSON/Parquet full scope/no-clobber, negative scan paths.
3. Метаданные локального NTFS: контрольный набор 20–30 тыс. файлов и длинное дерево 272 тыс. файлов, реальный счётчик прогресса, Esc, отсутствие частичного snapshot, SQLite `quick_check=ok` и сохранение исходных volume/entry counts; успешное повторное сканирование. Не вычислять медиа-хэши и не удалять исходные файлы.
4. Реальные разрешённые системные каталоги (например fonts) — только metadata-only read-only, **не** запускать весь системный том без отдельной оценки безопасности/размещения SQLite.
5. GUI: 1024×768, 1366×768 и 1320×760 **изменением размера окна**, RU/EN, light/dark, клавиатура, длинные пути, ошибки/модалы, wheel scrolling/Back; DPI 100/125/200% только если это реально возможно без нежелательного изменения чужой RDP сессии, иначе BLOCKED.
6. Чистые PNG: QA-only renderer F12 из PR69 с feature `qa-screenshots`, если код уже принят; учесть, что этот **инструментированный** EXE может отличаться от штатного hosted EXE. Критические действия также проверить на hosted EXE, а источник каждого скриншота указать отдельно. WGC timeout не должен заставлять пользоваться Paint/захватом чужих окон.
7. UX-метрики: для пяти сценариев записать click count, переходы focus, pointer travel **только если реально наблюдались**, время ответа и размеры viewport; прочие метрики BLOCKED, не подменять оценками.

**Выход W2:** `WIN11_GUI_LAUNCHED`, `WIN11_NTFS_SCAN_CANCEL_ROLLBACK`, `WIN11_SEARCH_EXPORT` и `WIN11_UX_ATLAS` независимо PASS/FAIL/PARTIAL; собранный redacted checkpoint. Перенос Win10 PASS на Win11 без запуска запрещён.

### W3 — PR58 P0 UX parity / feature freeze

1. Трассировать каждый основной экран/действие PR58: prototype mock → `atlas-service` → реальный egui widget/QA ID; сохранить эталон дизайна, без редизайна из Win32/SwiftUI.
2. Закрыть навигацию, scroll/selection preservation, реестр 60+, inspector, properties, search Boolean/global/disk scope, scan/progress/cancel, export/no-clobber и реальные ошибки на **Win10+Win11**.
3. Категории — явный **scope conflict**: мастер-план считает native category workflow P0, а текущая матрица PR69 помечает отсутствие UI как P1. Не понижать приоритет молча; либо реализовать, либо оформить документированное решение владельца о scope/RC. Аналитика (sunburst/slices) и local analyzer — отдельно P1, не считать их mock реальной функцией.
4. Для отсутствующих PR58 экранов статус `MISSING`, для экспериментальных — `DISABLED/POST_1_0` после явно принятого scope decision; не обещать видео-распознавание, точные копии, reclaimable bytes по metadata.

**Выход W3:** утверждённая P0 parity-matrix с screenshot pairs, всеми существенными P0 закрытыми, P1 явно классифицированы, без ложных claims; иначе только RC.

### W4 — Windows 10+11 установленная сборка, lifecycle и signing (P0)

1. Не путать отдельный egui QA preview ZIP с legacy Win32 installer: проверить, что **новый кандидат действительно устанавливает целевой egui продукт**. Документировать app identity, side-by-side policy и миграцию с RC3.
2. Чистая per-user установка, запуск/закрытие, Open/New, shell/Unicode, upgrade RC3→новый **неперезаписанный RC4+ prerelease**, data/sidecar preservation, uninstall без удаления пользовательских каталогов, clean reinstall; отдельные журналы Win10 и Win11.
3. Проверить Windows Defender/SmartScreen/авторизацию, отсутствие обязательного браузера/Node, no-clobber и права пользователя. Не ослаблять настройки ОС ради прохождения теста.
4. Подготовить Authenticode signing (секреты вне repo/логов) и независимый SHA/provenance verification; до подписания выпуск — только явно unsigned prerelease, не stable.

**Выход W4:** доказанная Win10/Win11 installed-egui lifecycle и подписанный Windows кандидат для stable, либо явный signing BLOCKED.

### W5 — CTF fidelity, real 3M perf и надёжность (параллельно W2–W4)

1. Оригинальная WhereIsIt на изолированном Win11 стенде открывает **созданный/отредактированный тестовый CTF**: read → edit → open/save оригинальной программой → независимое compare без потери полей. Проверять только поддерживаемый subset; никаких правдоподобных фальшивых PASS. Если приложение не предоставлено, BLOCKED.
2. Два ранее загруженных исторических CTF: провайдер байтов заблокирован; возобновить только авторизованный доступ, не просить повторную передачу без проверки существующих источников.
3. Валидация 3 046 027 записей — отдельный **закрытый read-only тест**, без размещения реального CTF, имени файла, его хэша и производных записей в CI. CTF/SQLite invariants, category/duplicate semantics, fallback/rollback.
4. На Win11 и эталонной системе измерить cold/warm p50/p95/RSS/CPU для search, import/export, responsive UI, cancel; источниковые SHA-проверки не отключать ради `p95 ≤ 150 ms`. Если SLO не выполнен — оптимизировать либо отдельно согласовать новое release SLO.

**Выход W5:** оригинальное CTF подтверждение или формально ограниченный supported subset, независимые real-data benchmarks с принятой метрикой, ни одной скрытой data-loss операции.

### W6 — macOS ARM+Intel physical QA и доверенный PKG/DMG (параллельно)

GitHub-hosted сборка ARM/Intel сама по себе не заменяет физический launch/Retina/VoiceOver, RU/EN, SQLite/CTF/metadata-only, update/uninstall. Использовать подписанный Developer ID и notarization/staple для stable; отдельно принять оба реальных типа Mac либо честно сузить поддерживаемую матрицу официальным решением до freeze. Missing physical hardware — `BLOCKED`.

### W7 — выпуск кандидата и final GO/NO-GO

1. Слить только проверенные вертикальные PR (включая релизные docs), затем прогнать полный exact-main SHA Linux/Windows/macOS CI + offline/privacy + RC3→new-RC upgrade. Отдельно принять Win10/Win11 GUI и физический macOS QA.
2. Freeze API/schema, P0 scope, known limitations, P1 off-by-default, README/install instructions; сверить VERSION, Cargo.lock, `THIRD_PARTY_NOTICES`, LICENSE, build identity, SHA256SUMS, provenance и подписи.
3. При неполных воротах допускается **только новый prerelease** (`rc.4` или позднее), без переноса существующего `v1.0.0-rc.3` tag и без `stable_ready=true`.
4. `release/ACCEPTANCE.json` менять на stable PASS лишь при независимых доказательствах всех обязательных критериев (или явном утверждённом scope revision). Создать immutable `v1.0.0` tag **только** после GO.

## 4. Матрица релизных ворот на 21 ноября

| Gate | Факт | Что закрывает следующий блок |
|---|---|---|
| G0/G1: PR58/source + CI | PASS source integration на `main ddd9215b` | Новые postmerge source SHA после PR69 |
| G2: настоящая egui UI parity | PARTIAL; PR69 draft | W0 + W2 + W3 |
| G3: scan/search/export/services | Win10 synthetic + NTFS targeted PASS, общая parity PARTIAL | W2 Win11 + W3 |
| G4: физический USB | **NOT_RUN по указанию владельца** (вне текущего Win QA), для полной прежней stable-spec BLOCKED | Отдельная hardware QA или явное новое scope decision; не маскировать PASS |
| G5: changed CTF оригинальной программой | BLOCKED | W5 |
| G6: реальная производительность 3M | PARTIAL; исторический p95 >150 ms | W5 |
| G7: privacy/no-clobber | Частично PASS, final package pending | W4/W7 |
| G8: native signed installers + upgrades | BLOCKED на подписях, CI upgrade ранее PASS | W4/W6 |
| G9: stable final review | NO-GO | W7 |

**Граница Win11:** успешный runner smoke ≠ GUI QA; локальный debug GUI ≠ hosted bundle; синтетический каталог ≠ real CTF compatibility; PASS Windows 11 ≠ PASS macOS; user deferred USB ≠ выполненный stable USB gate.

## 5. Минимальные следующие действия — без повторения пройденного

1. **Интегратор:** принять/обновить PR69 после точного CI и review; сохранить незакрытые UX parity задачи и не объявлять их закрытыми через merge.
2. **CI owner:** проверить и безопасно принять PR70, затем выполнить ровно один ручной trusted-main `win11-whereisit` smoke; подтвердить Online/Idle. Админ-runner — не для чужих PR.
3. **Windows 11 Codex/QA в интерактивной сессии:** взять exact-SHA hosted bundle из main; выполнить W2, сохранить приватные PNG/JSON, опубликовать только очищенный checkpoint с PASS/FAIL/BLOCKED.
4. **Параллельно:** CTF original-app correctness/performance и Windows egui installer lifecycle (W4/W5); macOS hardware/signing (W6).
5. **Интегратор:** пересмотреть G0–G9, сформировать новый prerelease RC4+ только после source freeze, stable GO — только по результатам W7.

После каждого блока: фактический SHA, run ID, версия ОС, бинарный SHA256, код/сценарий, наблюдение, снимок/лог (редактированный), PASS/FAIL/PARTIAL/BLOCKED и отдельный checkpoint. Никаких пустых микро-PR, force-push, переписывания release tags, реквизитов VPS или смешивания с другими проектами.
