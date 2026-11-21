# WhereIsIt AI — Codex самостоятельно получает программу на Win11

**21.11.2026, Windows 11 x64, Administrator.** Это инструкция установки для Codex, не отчёт о выполненной GUI/CTF-приёмке. Владелец предоставляет **только** частный CTF и легальный оригинальный WhereIsIt (при необходимости). Актуальный WhereIsIt AI и CLI агент берёт сам из GitHub Actions.

## 1. Откуда берём WhereIsIt AI

- Репозиторий: https://github.com/i-shutov/whereisit-ai
- Проверенный P0 category source PR #73: SHA `40c6b695958e49cbdad78affa314b139dd28d61b`
- Точный успешный CI: https://github.com/i-shutov/whereisit-ai/actions/runs/44891158178
- Артефакт `WhereIsItAI-egui-qa-windows-x64` — portable egui Windows x64 GUI, сопоставимый CLI, QA runner, лицензии и SHA/provenance.
- Артефакт `WhereIsItAI-egui-Win64-installed-QA-unsigned` — **отдельный** egui Win64 Inno QA установщик.
- Это **unsigned QA**, не стабильный 1.0. Если актуальный PR73 HEAD изменился, агент обязан найти успешный exact-head hosted CI и заменить **оба** значения: SHA и run ID. Нельзя смешивать сборки разных SHA.

## 2. Подготовка Git и GitHub CLI в интерактивной Administrator-сессии

Открыть PowerShell от Administrator **в RDP**, а не в служебной GitHub runner-сессии. Вывести список доступных команд:

~~~powershell
Get-Command git.exe, gh.exe, powershell.exe, py.exe, codex.exe -ErrorAction SilentlyContinue |
    Select-Object Name
~~~

Если Git или GitHub CLI отсутствуют и установка согласована с владельцем:

~~~powershell
winget install --id Git.Git --exact --source winget
winget install --id GitHub.cli --exact --source winget
~~~

После установки открыть новый PowerShell (обновление PATH). При отсутствии winget пользоваться только проверенными официальными дистрибутивами, не произвольными зеркалами. `gh` для GitHub Actions artifacts обычно требует входа:

~~~powershell
gh auth status
gh auth login --web
~~~

Вторую команду выполнять **только если не авторизован**; завершить вход через браузер. Токены не показывать Codex и не писать в GitHub issue или журналы.

## 3. Самостоятельно получить исходники и два артефакта

Сначала отдельный доверенный checkout `main`, вне папок FlowBridge/TikZarium и вне частного CTF:

~~~powershell
git clone https://github.com/i-shutov/whereisit-ai.git C:\WhereIsItQA\trusted-main
git -C C:\WhereIsItQA\trusted-main rev-parse HEAD
~~~

При существующем checkout и незакоммиченных изменениях — не перезаписывать, выбрать **новую** папку. Для проверки portable GUI **сборка Cargo не требуется**, используется готовый EXE из CI. Подтвердить наличие скрипта:

`tools/qa/windows11-egui/Get-VerifiedWin64QABundles.ps1`

Выполнить в PowerShell 5.1/7, где доступен `gh`:

~~~powershell
$sha = '40c6b695958e49cbdad78affa314b139dd28d61b'
$runId = '44891158178'
$qaRoot = 'C:\WhereIsItQA\Artifacts\win64-pr73-40c6b695'
powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'C:\WhereIsItQA\trusted-main\tools\qa\windows11-egui\Get-VerifiedWin64QABundles.ps1' -RunId $runId -ExpectedCommit $sha -DestinationRoot $qaRoot
~~~

Скрипт сверяет `gh run view` с точными `headSha`, `completed/success` и workflow name, скачивает **только два названных** артефакта через `gh run download`, проверяет checksum ZIP/preview provenance, затем отдельно проверяет SHA, source SHA и run ID установщика уже существующим `Verify-Win64EguiCandidate.ps1`. Нельзя перезаписывать существующую папку.

Успешный bootstrap: `WIN64_QA_BUNDLES_DOWNLOADED_VERIFIED: true`, `PREVIEW_ARCHIVE_SHA256_VERIFIED: true`, `INSTALLER_MANIFEST_SHA256_VERIFIED: true`. **На этом этапе ничего не устанавливается и не запускается** — это намеренно.

Структура:

~~~text
C:\WhereIsItQA\Artifacts\win64-pr73-40c6b695\
  preview\
    WhereIsItAI-egui-preview-...-windows-x64.zip
    SHA256-windows-x64.txt
    provenance-windows-x64.json
  installer\
    WhereIsItAI-egui-win64-QA-...-setup.exe
    SHA256SUMS.txt
    provenance.json
~~~

Если `gh` недоступен, владелец может скачать эти **точные два артефакта** из Actions UI. Codex обязан выполнить те же проверки; не приписывать ручной загрузке статус автоматического скачивания.

## 4. Запуск полученной программы

**Portable Win64 egui GUI + CLI:** выполнить доверенный launcher из `main` в интерактивной сессии Administrator:

~~~powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'C:\WhereIsItQA\trusted-main\tools\qa\windows11-egui\Invoke-VerifiedWin11EguiRdpQA.ps1' -ArtifactDirectory "$qaRoot\preview" -ExpectedCommit $sha -DiskCount 60
~~~

Launcher сверяет внутренние SHA GUI/CLI, извлекает EXE и запускает нативный egui на 60 синтетических дисках. Часть проверок интерактивная: **CLI PASS не означает GUI PASS**.

**Установленный egui Win64 QA:** в `$qaRoot\installer` находится отдельно проверенный unsigned Inno Setup EXE. В RDP после проверки издателя/предупреждений Windows можно установить его в отдельную QA-идентичность, затем проверить launch/restart/uninstall по `docs/qa/WIN11_WIN64_EGUI_MANUAL_CHECKLIST_20261121.md`. Downloader никогда не запускает setup автоматически и не отключает защиту Windows.

**Для настоящего CTF:** извлечь verified preview ZIP в отдельную приватную папку; в нём лежит `whereisit-ai-cli.exe`. Перед запуском проверить его SHA из `QA_MANIFEST.json`. Затем выполнять `tools/qa/offline_ctf_v3.py` с `--expected-sqlite-version 3.54.1` и собственным частным CTF read-only (PR #76 уже интегрирован); никакие исходные CTF не копировать в Git-репозиторий.

## 5. Откуда берётся оригинальная WhereIsIt

**Это другое приложение** — не часть GitHub-сборки WhereIsIt AI. Codex сначала проверяет существующую установку в Administrator-сессии и обычных директориях программ. Если отсутствует, пользователь предоставляет **свой лицензионный установщик и лицензию локально**, после чего Codex может установить оригинальную программу по согласованию с владельцем. Не качать и не запускать сомнительные архивные зеркала автоматически, не обходить активацию. Если оригинал установить не удалось — `BLOCKED_ORIGINAL_APP`, а не выдуманный PASS.

Приватный `*.CTF` пользователь также доставляет на Win11 сам. Агент не размещает его в исходниках, артефактах, чужих проектах или общедоступных отчётах; изменения только на копиях.

## 6. Продолжение независимой приёмки

После `BUNDLES_ACQUIRED` перейти к `docs/qa/WIN11_CODEX_REAL_CTF_ORIGINAL_ACCEPTANCE_20261121.md`. Раздельные ворота: `GUI_LAUNCHED`, `WIN64_INSTALLER_LIFECYCLE`, `REAL_CTF_READONLY`, `MODIFIED_CTF_OPENED_IN_ORIGINAL_APP`, `3M_SEARCH_P95`. Источник CTF и исходные имена/пути/хэши/лицензии не публиковать. Stable `1.0` остаётся NO-GO до независимой полной приёмки.
