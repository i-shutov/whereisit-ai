# WhereIsIt AI — интегральный checkpoint 2026-11-13

**Назначение:** самодостаточный проверяемый срез проведённых работ, решений, релизных доказательств и исходного кода для продолжения разработки. Это **не** новый релиз, не сертификат полной совместимости и не бэкап приватных пользовательских каталогов.

## Где начинать

1. `reports/INTEGRATED_REPORT_RU.md` — единая аналитическая сводка проекта и реальной приёмки.
2. `reports/NEXT_STEPS_AND_GATES_RU.md` — незакрытые условия и последовательный план завершения.
3. `reports/TECHNICAL_HANDOFF.md` — точная процедура возобновления, тестовые команды и правила безопасности.
4. `metadata/STATE.json`, `evidence/EVIDENCE_INDEX.json`, `evidence/QA_MATRIX.csv` — машиночитаемые статусы, SHA, прямые ссылки и матрица проверок.
5. `source/WhereIsItAI_main_cabdb3d2_tree_00c1e20a_source.zip` — **полное публичное исходное дерево `main`**, Git tree SHA `00c1e20ab5779475610275e1292cd2c5e333b618` (580 элементов); сохранено из privacy-gated офлайн-кита, где commit SHA другой (`e87e06f3...`), но дерево **идентично** HEAD `cabdb3d2...`.
6. `source/WhereIsItAI_PR57_draft_93d7e525_source.zip` — **отдельный** полный исходный срез *не слитого* UX PR #57, Git tree `4f09c2b6175d12899881ad95381ed9d95c92e921` (631 элемент). Не подменять им `main`.
7. `refs` — извлечённые из указанных исходных архивов авторитетные документы и runbooks, без правки.

## Проверка

```sh
sha256sum -c MANIFEST.sha256
unzip -t source/WhereIsItAI_main_cabdb3d2_tree_00c1e20a_source.zip
unzip -t source/WhereIsItAI_PR57_draft_93d7e525_source.zip
```

В подкаталоге `source` нет приватных CTF/SQLite/имен файлов с USB-носителей, ключей подписи и пользовательских данных. Существующие unsigned/adhoc установщики размещены в релизе GitHub; отдельный privacy-gated Linux offline kit со строго проверенным SHA-256 выгружен в соседний файл на Google Drive (путь и id указаны в `metadata/STATE.json`).

**Ключевой принцип:** published `v1.0.0-rc.3` tag `5d91ba12...` и later corrected `main` `cabdb3d2...` — **разные коммиты/деревья**. Не утверждать, что поздний privacy gate уже входил в опубликованные бинарные RC3-активы. Не перемещать существующий tag.
