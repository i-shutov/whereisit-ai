# WhereIsIt AI — категории Rust/egui, P0 Win64 QA checkpoint

**Создано:** 2026-11-21. **Ветка:** `feature/egui-categories-win64-20261121`, [PR #73](https://github.com/i-shutov/whereisit-ai/pull/73) — Draft. **База:** `main e9f65bbb` с установленным Win64 Rust/egui QA-кандидатом из PR #72. **Вердикт:** SOURCE IN PROGRESS / GUI NOT_RUN / stable 1.0 NO-GO.

## Почему это P0

Мастер-план 1.0 требует работающие категории в нативном интерфейсе Windows x64. PR #58 остаётся UX-эталоном, но его in-memory демонстрация не является реальным SQLite. Старый статус `MISSING / P1` в PR69 был конфликтом scope — он исправлен в текущей матрице на **PARTIAL / P0**, а не молча понижен.

## Контракт реального SQLite, без нового поискового движка

| Операция | Rust API | Действие GUI, безопасность |
|---|---|---|
| `categories` | `atlas-core` через `atlas-service` | Асинхронная загрузка справочника; при ошибке список очищается и навигация/изменения блокируются |
| `create_category` | SQLite v3, name+optional parent | Ввод 1–80 печатных символов, выбор родителя с полным путём и ID |
| `entry_category_ids` | Чтение `entry_categories` по ID записи | Показываются **прямые** назначения; никаких догадок об унаследованном присутствии |
| `assign_category` | INSERT OR IGNORE, FK-проверки | Только явное назначение выделенной записи после подтверждённой загрузки |
| `unassign_category` | DELETE из `entry_categories` по двум ID | Снимается **только связь категории**; файл, папка, запись каталога и CTF не удаляются; связь можно назначить снова |
| `category_id:N` | Параметризованный положительный ID и recursive CTE | Уникальная категория независимо от повторяющегося имени; включены дочерние категории и вложенные записи папок |
| `category:NAME` | Устаревший публичный поисковый синтаксис | Намеренно объединяет все одноимённые категории; теперь так же включает их дочерние категории; не используется для выбора одной конкретной категории |

**Семантика:** parent category includes assigned child categories; category assigned to a catalog directory makes its recorded descendants searchable. Отображаемые категории на выбранной записи — **только прямые** связи; унаследованное соответствие в этом списке не отмечается. Нельзя интерпретировать совпадения как физическое подключение USB или идентичность файла.

## Проверки кода

Синтетический Rust тест `apps/desktop-egui/src/categories.rs` создаёт локальный временный SQLite и маленькую сгенерированную папку, выполняет metadata-only scan, создание и назначение, проверяет два разных category ID с одинаковым именем «Видео», наследование по дочерней категории, корректную валидацию `category_id` и `category:NAME`, снятие связи, повторное снятие (idempotent), восстановление и сохранность записи файла. Дополнительные тесты проверяют имена/родителей, циклические отображаемые пути, навигацию назад и запрет редактирования до подтверждения принадлежности.

**Источник результатов:** exact-head CI **обязателен для каждого нового commit SHA**. На промежуточном коммите `4f976521` Rust workspace tests/fmt/strict Clippy PASS. Финальные source и Win64 installed-egui integration результаты по более поздним коммитам нужно записать только после завершения их собственных workflow. Это не физическая GUI-приёмка.

## Независимые аудиты и закрываемые дефекты

1. Первое независимое ревью выявило P0: `category:имя` в GUI ошибочно соединял разные категории с одинаковым названием. Исправлено адресацией `category_id` с bind-параметром и тестами.
2. Второе ревью подтвердило исправление P0 и обратимость на синтетике, но выявило P1: одинаковые имена визуально неразличимы, справочник устаревает после ошибки, семантика двух операторов различается, нет контекста конкретного диска/пути, и назначение папке не поясняет наследование. В PR внесены ID и полный путь, fail-closed загрузка, согласованная семантика и предупреждение.
3. Остаётся открытым: PR58 визуальная/эргономическая parity, полная RU/EN/keyboard/a11y матрица, измеренные pointer/click/time показатели, масштабный p95 и реальный Win11 установленный GUI.

## W2/W3 — протокол интерактивной Win11/Win64 приёмки

Использовать **только** SHA/provenance-верифицированный egui Win64 артефакт из GitHub-hosted CI, в отдельном неадминистративном профиле. Каталог и медиа — генерируемые; не открывать личные CTF/SQLite. Зафиксировать следующий минимальный набор с screenshot pairs PR58/egui и PASS/FAIL/BLOCKED/NOT_RUN:

| ID | Реальное действие | Критерий |
|---|---|---|
| CAT-01 | Открыть категорийное окно на 60+ синтетических дисках | Не блокирует UI, компактность 1024×768/1366×768, RU/EN |
| CAT-02 | Создать две категории «Видео» без родителя | Различимы по `#ID`, нет объединения выборок |
| CAT-03 | Создать дочернюю `Видео / Документальные` | В списке и селекторе родителя виден полный путь+ID |
| CAT-04 | Выделить одноимённый файл на разных дисках | Отображаются ID записи, номер тома и относительный путь |
| CAT-05 | Назначить категорию выбранной записи | Видна подтверждённая прямая связь, а не предположение |
| CAT-06 | Снять связь и назначить снова | Исчезает/возвращается только ссылка; исходная запись и файл сохраняются |
| CAT-07 | Назначить категорию папке | Отображается предупреждение; поиск включает вложенные записанные элементы |
| CAT-08 | Найти по `category_id:N` и `category:NAME` | ID-выборка точная, name-фильтр честно объединяет ветви; наследование согласовано |
| CAT-09 | Ошибка загрузки, неверный ID, отсутствующая запись | Никаких старых списков под новым запросом, управляющие действия неактивны |
| CAT-10 | Поиск по категории → выбор → Back; прокрутка и клавиатура | Возвращаются предыдущая область поиска/выделение; не заявлять пройденное без наблюдения |

## Релизные ворота, которые этот PR не закрывает

Физическая приёмка реальных Win10/Win11 GUI и USB hardware; PR58 полный функциональный UX, CTF modified-output в исходной WhereIsIt, 3 046 027 записей / `p95 ≤ 150 ms`, подписание Windows Authenticode, macOS Developer ID/notarization и физический запуск ARM/Intel. Сохранить `release/ACCEPTANCE.json: stable_ready=false`. Никакого нового stable tag/переноса `v1.0.0-rc.3` без GO.

## Windows PowerShell 5.1 category smoke — new QA lane

The existing synthetic Win10/Win11 QA runner now supports an opt-in
-CategoryChecks switch. It uses the accepted BOM-free UTF-8 stdin JSON-lines
native CLI transport and generated SQLite to cover homonymous IDs, direct
assignment, exact category-ID search, parent-category inheritance, unassignment
and relinking, invalid IDs, and final catalog integrity.

GitHub-hosted Windows CI invokes the script with six synthetic disks and
-SkipGui -CategoryChecks; the same packaged script supports interactive Win11
with -DiskCount 60 -ManualReview -CategoryChecks. Manual category GUI prompts
are NOT automatically accepted. The separate sidecar byte-immutability check
still executes after category mutations. No private CTF or USB hardware is used.

Source change is not a PASS until exact new commit SHA CI is complete.
