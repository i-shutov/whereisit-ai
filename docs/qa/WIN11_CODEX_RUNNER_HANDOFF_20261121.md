# WhereIsIt AI — Windows 11 / Codex: safe execution lanes

**2026-11-21. Scope: Win64, Rust/egui 1.0. Status: preparation only.** This document is a runnable handoff, **not** proof that the Win11 runner/Codex/GUI have been exercised.

## What is and is not connected

- GitHub self-hosted Windows runner is a GitHub Actions job executor. The existing runner named win11-whereisit belongs to a Windows 11 Pro host and has historically run as a local **administrator service account**. It is not an RDP, shell, or visual control channel for this chat.
- The owner has installed Codex CLI. The owner's **Administrator account** is approved for Win11 RDP, PowerShell, native egui GUI QA and Codex. Availability to the service session or current interactive shell has not been verified. Installation does not establish authentication, CLI version, GUI automation or a shell connection to this chat.
- The authorized existing admin workflow is .github/workflows/win11-runner-smoke.yml. It is intentionally manual workflow_dispatch, exact trusted main, repository+runner+machine+service identity checks, no code checkout, no third-party execution, no untrusted PR. An updated smoke reports whether the service can *find* the Codex command **without executing it**. Logs are limited to booleans, command names and OS build; do not publish host/account names, user paths, tokens or CTF.

Do not automatically execute unreviewed public PRs with Administrator privileges through the self-hosted service. That is distinct from the owner's authorized **interactive Administrator** QA and Codex work in dedicated WhereIsIt directories. A service-session headless job does not constitute physical GUI/RDP acceptance.

## Lane 1: inspect the existing runner safely

After the reviewed smoke workflow is merged to main, use GitHub UI. In **Run workflow → Use workflow from**, select **main**, not a planning branch (the screenshot's error was caused by the branch selection):

1. Actions -> **Windows 11 VPS runner smoke** -> **Run workflow**, select **main**.
2. Inspect the exact run result, GitHub source SHA, and the log markers RUNNER_SMOKE_PASS, RUNNER_HOST_MATCH, SERVICE_ACCOUNT_MATCH, CODEX_CLI_VISIBLE_TO_SERVICE, CODEX_EXECUTED: false.
3. If the job does not start, confirm Settings -> Actions -> Runners reports win11-whereisit Online/Idle and that no other project's services were touched.
4. Do not interpret Codex visibility in a service PATH as interactive user Codex authorization or physical Win11 GUI QA.

The current GitHub connector can inspect workflows and commits but cannot dispatch this manual workflow; a person with repository permission must use the Run workflow control. **Do not** change the trigger to push or pull_request to work around this.

## Lane 2: existing interactive Administrator account — Codex QA

**Owner directive:** use the existing Administrator account for Codex and Win11 testing. Do not require a separate standard/non-admin account. Keep a separate clean WhereIsIt checkout/worktree, outside the FlowBridge and TikZarium projects.

Under the owner's **Administrator** account in an interactive Win11 session, open PowerShell **7**. Checkout the exact SHA of the code to review. For category PR #73 this is currently 7bfdfb156d004f6e281e626ecd19e816a44dfe5e; refresh it before use. The checkout must be clean.

The launcher must be taken from a **separately reviewed trusted tooling checkout** (for example this PR after acceptance); the target under audit may be an older PR #73 checkout that does not yet contain the launcher. The following command is a **preflight** and does not launch an agent. Replace example paths with two separate local worktrees; do not check out unreviewed code into the trusted tooling directory:

~~~powershell
$toolsRepo  = 'C:\WhereIsItQA\trusted-tools'
$targetRepo = 'C:\WhereIsItQA\categories-review'
pwsh -NoProfile -File "$toolsRepo\tools\qa\windows11-egui\Invoke-Win11CodexReadOnlyAudit.ps1" -RepositoryPath $targetRepo -ExpectedSha '7bfdfb156d004f6e281e626ecd19e816a44dfe5e' -Focus categories
~~~

After reviewing the selected task, add -Execute to run the audit. The launcher **allows Administrator** but refuses a service/CI process, a noninteractive session, dirty or wrong source checkout, and missing Codex. Codex uses **read-only** sandbox, ignores user/project config and does not persist an agent rollout; the final report stays in the Administrator user's LOCALAPPDATA/WhereIsItAI-QA/CodexAudits directory. Inspect and redact before sharing. The launcher does not push, commit, alter other projects, install software or perform GUI input.

Independent read-only review focuses: categories, win64-installer, release-risks. Run them from separate Administrator terminal sessions if desired. These reviews are not substitutes for automated Rust tests or human-independent GUI acceptance. Do not claim agents were launched merely because this documentation exists.

## Lane 3: interactive native Win11 RDP GUI / SQLite QA

Take the exact-SHA hosted Windows native egui preview bundle or isolated Win64 installed QA candidate from **successful GitHub-hosted CI** (not untrusted uploads). Check the Actions run ID, source SHA and SHA-256/provenance before executing any unsigned software.

For the side-by-side installed candidate, follow:

- tools/qa/windows11-egui/Verify-Win64EguiCandidate.ps1
- docs/qa/WIN11_WIN64_EGUI_MANUAL_CHECKLIST_20261121.md
- docs/qa/EGUI_CATEGORY_P0_CHECKPOINT_20261121.md (in PR #73 while draft)

For preview mode use tools/qa/windows10-egui/Invoke-WhereIsItEguiQA.ps1 (Win11 supported) under the owner's **interactive Administrator** account. The script creates synthetic volumes/catalog only, checks SQLite/search/exports and opens a visible egui window; the -ManualReview option requires actual observations. The automated hosted Windows CI does NOT establish GUI parity, physical USB or 3M real CTF performance.

Only provide **redacted aggregate findings** to GitHub: source SHA, workflow run ID, installed EXE hash, OS build/architecture, per-scenario PASS/FAIL/BLOCKED, reviewed screenshot atlas and no personal paths. No private CTF bytes or metadata in public artifacts.

## Lane 4: future dedicated runner for agent development (not implemented)

Unattended Codex under the existing Administrator **runner service** is not enabled by this PR. It would need a separately reviewed trusted-main manual workflow, task scope, approvals and isolation from other project directories. The owner's **interactive Administrator Codex** testing needs no separate user account. Do not automatically execute unreviewed public PR code through the runner service.

## Current release gates

The Win64 egui installed QA smoke passed in GitHub-hosted CI and PR #72 is in main. PR #73 exact source SHA 7bfdfb15 passed Linux/Windows/macOS ARM+Intel, offline, integration and upgrade workflows; independent interactive Win11 categories still **NOT_RUN**, not a blocker that may be silently waived for stable. Stable 1.0 remains NO-GO pending GUI/USB, changed CTF in original app, real 3M p95, signed installers, and other master-plan gates.

## 2026-11-21: first manual runner smoke failed before script execution

- Exact run: https://github.com/i-shutov/whereisit-ai/actions/runs/44885465184 ; main HEAD at run was `7a05e8b23136ee8bc8d47c0cc0c6fc4d4279c3ed`.
- The job was assigned to the expected `win11-whereisit` runner on the correct Windows 11 host. It failed in the first `run` step **before any PowerShell script body executed**: Windows PowerShell 5.1 refused the GitHub-generated temporary `.ps1` with `PSSecurityException: running scripts is disabled on this system`.
- Root cause: using `shell: powershell` inherited the Windows machine's script Execution Policy. Not a runner registration, identity, Rust, Codex or administrator-account failure. Host account checks did not yet execute.
- Hotfix already committed directly to trusted `main`: [`2b6e63a`](https://github.com/i-shutov/whereisit-ai/commit/2b6e63a948405394a26589fbf7ba1119d2cc0bc4). All manual smoke script steps now use custom `shell: powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File {0}`. `-ExecutionPolicy` is **process-scoped**; do not change `MachinePolicy`, `LocalMachine`, Registry, users, service configuration or the two other projects' runners.
- PR #74 mirrors that shell fix and adds a hosted static regression guard. It does **not** enable running Codex on the admin service or PR-triggered jobs.
- For a real verification, use **Actions → Windows 11 VPS runner smoke → Run workflow → Use workflow from: main**. A rerun of the failed historical attempt does not establish that the newly committed workflow source was used. Record the new run ID and exact source SHA. Require `RUNNER_SMOKE_PASS` with exit code 0; the Codex visibility step is only available after PR #74 is merged and checks pass.
- If the new run still fails with a policy error, inspect `Get-ExecutionPolicy -List` locally for restrictive Group Policy (`MachinePolicy`/`UserPolicy`) and stop for a scoped decision. A process-only bypass cannot override enforced Group Policy. Do not globally disable PowerShell protection to make QA green.
