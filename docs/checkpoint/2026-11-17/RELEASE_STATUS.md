# WhereIsIt AI — 17.11.2026 интегральный релизный checkpoint

**Решение:** SOURCE INTEGRATION IN PROGRESS / STABLE 1.0 **NO_GO**.
Этот документ сохраняет состояние при продолжении после зависших/прерванных сеансов.
Все SHA и CI ниже относятся к публичному синтетическому тестированию — приватные
CTF, пользовательские SQLite, исходные пути и лицензии не публикуются.

## Подтверждённые завершённые крупные блоки

| Блок | PR / точная версия | Доказательство | Статус |
|---|---|---|---|
| R0 source UX + native | PR #58 (прототип) + PR #57 в PR #59 | `main=ff87d109` | SOURCE MERGED |
| R2 современный native Rust/egui | PR #62, merge `81f0491267ab0b63109d99a470c3c0ff8ba37d9b` | Win/Linux/Mac ARM+Intel CI + install upgrade 3/3 | SOURCE PASS |
| R3 реальный SQLite export | PR #63, merge `405b4ccbc534db73cc6f986b77c8ba6e1b5a5e9e` | exact PR head `9e8caaeb`: native CI [44035800572](https://github.com/i-shutov/whereisit-ai/actions/runs/44035800572), actual installed upgrade [44035800600](https://github.com/i-shutov/whereisit-ai/actions/runs/44035800600), offline QA [44035800563](https://github.com/i-shutov/whereisit-ai/actions/runs/44035800563) | SOURCE PASS |

## Незакрытые задачи следующего исполнения

| Блок | Текущее состояние | До принятия |
|---|---|---|
| R4 scan/progress/cancel | [PR #64](https://github.com/i-shutov/whereisit-ai/pull/64), commit `c105be84848521b923bf708223d8f7628fa7a828` | Exact-source Mac Intel native build; Windows+Mac manual cancel/rollback test; source merge в main |
| R5 тестовые GUI-пакеты egui | [PR #65](https://github.com/i-shutov/whereisit-ai/pull/65), commit `e3fe5e49e9243c75405050c3cdc6d7a271397b25` | macOS Intel native + upgrade smoke, затем source merge; ZIP только QA, без Apple notarization |
| R6 physical native UX | [Issue #66](https://github.com/i-shutov/whereisit-ai/issues/66) | Реальные Win10/11 x64 и macOS ARM/Intel, DPI/Retina, RU/EN, USB unplug, search, scan, cancel, export |
| R7 release | НЕТ независимых подписанных бинарных 1.0 | Authenticode + Developer ID, notarization/staple, full compatibility/perf gates |

## Известные запреты и ограничения

- Каталогизатор всегда офлайн; React-прототип PR #58 — UX-эталон, Node/Express не входят в native installer.
- Настоящий egui GUI **написан** как preview, но визуально-поведенческая parity PR #58 и manual GUI acceptance **не** пройдены.
- R3 CSV/JSON/Parquet экспорт использует полную SQLite выборку, не только видимую страницу. Запрет перезаписи принят.
- R4 сканирует метаданные; не читает файл ради видео/хэша и не удаляет пользовательские файлы.
- Исторические CTF, original-app modified CTF и смысл opaque 0x10 отложены до полной сборки, не считать PASS.
- Производительность real-data query p95 <=150 ms, физическая идентификация USB и trusted signing остаются open.
- Стабильный релиз `v1.0.0` и флаг `stable_ready=true` запрещены до независимых доказательств.

## Порядок следующего блока

1. Следить за exact-SHA оставшимися Mac Intel CI R4/R5; устранять ошибки, не обходя safety gates.
2. После PASS сначала merge [PR #64](https://github.com/i-shutov/whereisit-ai/pull/64), затем независимый [PR #65](https://github.com/i-shutov/whereisit-ai/pull/65).
3. Повторить postmerge CI на точном `main`, сверить binary artifacts SHA/provenance.
4. Передать отдельно созданные QA-preview ZIP на настоящие машины по [issue #66](https://github.com/i-shutov/whereisit-ai/issues/66).
5. Сохранить следующий checkpoint без публикации личных данных, затем вернуться к CTF после завершения приложения.

Снимок сформирован как redacted public evidence, не заменяет полные CI и manual QA.
