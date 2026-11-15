# WhereIsIt AI — закрытие 1.0: приоритеты, блокеры, план работ

**Приёмочный принцип:** научно/инженерно разделять «реально протестировано», «синтетическая CI», «только код», «не проверено на железе», «источник недоступен».

## Приоритетная последовательность крупных блоков

### G0 — единый продукт и воспроизводимое состояние (сейчас)
- Источник интеграции `main=cabdb3d2...`, tree SHA `00c1e20...`; релизный tag `v1.0.0-rc.3=5d91ba12...` сохранён неизменным.
- PR #57 — отдельная несмерженная ветка `ux/search-first-disk-locator-20261101`, HEAD `93d7e525...`; проверка 4/4 native CI и 3/3 upgrade есть, ручной QA отсутствует.
- Держать строгий приватностный gate в publisher и offline kit; исходные пользовательские CTF/SQLite не грузить в GitHub Actions/Drive checkpoint.
- Этот архив — новая автономная точка передачи (не копия старого handoff с устаревшим main).

### G1 — UX приёмка PR #57 и физический поиск диска
- Отработать **один интегральный** UI/физический QA проход: Win10/Win11 и Mac ARM64/Intel, 880/1100/1180/1366px, 100/125/200% DPI и Retina, keyboard navigation и accessibility, реальные USB plug/unplug, копии UUID/drive letters, false-positive connected.
- Проверить «найти файл → найти полочный/USB-диск → показать другие записанные экземпляры» без подмены названий, без угадывания диска, без выдачи metadata candidates за идентичные байты.
- Отклонить GUI/UX, если table+inspector создают лишние прыжки взглядом или мешают скроллу. Не мержить PR57 до согласованной приёмки или явной маркировки ограничений.
- После замеров связать сценарий с measurable mouse clicks/travel и пройти visual atlas.

### G2 — бинарная совместимость CTF и исторические форматы
- Прогнать modified/new CTF в **оригинальной WhereIsIt** на изолированной Windows, приложить log/screenshot, без лицензии в публичном репозитории.
- При разрешённом доступе локально прочитать уже предоставленные 2 исторические CTF, получить независимый SHA, формат, full roundtrip, 15-field export parity и error coverage; никаких личных путей в общие отчёты.
- Investigate opaque `0x10` только на контролируемых синтетических паттернах + authoritative/original-app validation; никогда не писать недоказанный decoder.

### G3 — performance SLO и progress/cancel
- 3M exact indexed source: full-source SHA per read оставить. Сравнить оптимизацию обработки метаданных без снижения гарантий; измерять server-side p50/p95, first page vs COUNT, cold vs warm, CPU/RSS, 13+ Boolean predicates, категория 374k, 32 757 duplicate candidates.
- Сложный поиск/дубликаты не должен блокировать native UI. Реализовать реальную отмену долгого I/O, если текущая кнопка Stop waiting отменяет только показ старого результата.
- Проводить одни и те же сценарии на Linux и физических Win/Mac; принять ≤150ms только по реальному бюджету, а не синтетическому бенчу.

### G4 — security/packaging/release
- На новом candidate exact SHA: 4/4 native CI, 3/3 installed RC-to-next upgrade, full source tree parity, offline kit SHA, privacy allowlist, reproducible manifests, no secrets/private CTF.
- Windows Authenticode, macOS Developer ID and notarization/Stapler/Gatekeeper на чистых машинах.
- Новый tag только после полного принятия; старый RC3 не перезаписывать. При нехватке подписи/ручных доказательств выпускать RC, но **не stable**.

## Чего не делать

- Не начинать заново разработку CTF pinned mirror, annotation CAS и duplicate export — всё это уже интегрировано в main RC3.
- Не считать green CI визуальной приёмкой или фактом подключённого USB.
- Не делать read-only CTF writable, не перезаписывать его, не сливать два несвязанных annotation identity без явного разрешения конфликта.
- Не отправлять private raw CTF, SQLite/Parquet, source SHA/private paths и лицензионные ключи в публичные артефакты.
- Не затягивать проект бесконечными микро-PR: крупные завершённые вертикали с тестами, checkpoint и явным решением о GO/NO_GO.

## Готовые команды для следующего исполнителя

```sh
# из независимого источника, сверяя exact SHA и Git tree
 git clone https://github.com/i-shutov/whereisit-ai.git
 cd whereisit-ai
 git checkout cabdb3d2b139a67729a795ba9c34e4d3144c9c9f
 python3 tools/qa/check_public_evidence.py
 python3 -m unittest discover -s tools/qa -p 'test_public_evidence_gate.py' -v
 python3 tools/dev/verify_release_identity.py
 cargo test --workspace --locked
 cargo fmt --all -- --check
 cargo clippy --workspace --all-targets --locked -- -D warnings

# для текущего UX-PR57 работайте в его ВЕТКЕ, не подменяя main
 git fetch origin ux/search-first-disk-locator-20261101
 git checkout 93d7e525dc0433b6790a99250030c9c7fe09ca9a
```

**Локальные offline source.zip в архиве уже имеют полные деревья**. Перед запуском на них подтверждайте соответствие значений `metadata/STATE.json` и `evidence/EVIDENCE_INDEX.json`.
