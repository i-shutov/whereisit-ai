# WhereIsIt AI — Windows 11 / Codex: safe execution lanes

**2026-11-21. Scope: Win64, Rust/egui 1.0. Status: preparation only.** This document is a runnable handoff, **not** proof that the Win11 runner/Codex/GUI have been exercised.

## What is and is not connected

- GitHub self-hosted Windows runner is a GitHub Actions job executor. The existing runner named win11-whereisit belongs to a Windows 11 Pro host and has historically run as a local **administrator service account**. It is not an RDP, shell, or visual control channel for this chat.
- Codex CLI is installed according to the owner, but whether it is available to the service account or a separate non-admin interactive account **has not been verified**. Installation does not establish authentication, CLI version, GUI automation or a shell connection to this chat.
- The authorized existing admin workflow is .github/workflows/win11-runner-smoke.yml. It is intentionally manual workflow_dispatch, exact trusted main, repository+runner+machine+service identity checks, no code checkout, no third-party execution, no untrusted PR. An updated smoke reports whether the service can *find* the Codex command **without executing it**. Logs are limited to booleans, command names and OS build; do not publish host/account names, user paths, tokens or CTF.

GitHub recommends avoiding long-lived self-hosted runners for untrusted public PRs. Do not weaken these guards. Do not use this admin runner to run codex exec, compile PR code, install QA binaries or automate a user desktop.

## Lane 1: inspect the existing runner safely

After the reviewed smoke workflow is merged to main, use GitHub UI:

1. Actions -> **Windows 11 VPS runner smoke** -> **Run workflow**, select **main**.
2. Inspect the exact run result, GitHub source SHA, and the log markers RUNNER_SMOKE_PASS, RUNNER_HOST_MATCH, SERVICE_ACCOUNT_MATCH, CODEX_CLI_VISIBLE_TO_SERVICE, CODEX_EXECUTED: false.
3. If the job does not start, confirm Settings -> Actions -> Runners reports win11-whereisit Online/Idle and that no other project's services were touched.
4. Do not interpret Codex visibility in a service PATH as interactive user Codex authorization or physical Win11 GUI QA.

The current GitHub connector can inspect workflows and commits but cannot dispatch this manual workflow; a person with repository permission must use the Run workflow control. **Do not** change the trigger to push or pull_request to work around this.

## Lane 2: separate local non-admin interactive Codex

In Windows Settings, use a **standard user account** for software QA rather than the existing administrator service identity. Do not migrate stored Codex credentials; sign in normally under that separate user if needed. Use an independent clean clone/worktree of WhereIsIt AI, never the working directories of FlowBridge or TikZarium.

Under this standard user, in an interactive Win11 session, open PowerShell **7**. Checkout the exact SHA of the code to review. For category PR #73 this is currently 7bfdfb156d004f6e281e626ecd19e816a44dfe5e; refresh it before use. The checkout must be clean.

The following command is a **preflight** and does not launch an agent. Substitute your actual local clone path and exact HEAD:

~~~powershell
pwsh -NoProfile -File .\tools\qa\windows11-egui\Invoke-Win11CodexReadOnlyAudit.ps1 -RepositoryPath . -ExpectedSha <40-hex-SHA> -Focus categories
~~~

After reviewing the selected task, add -Execute to run the audit. The launcher refuses admin elevation, a service/CI session, dirty or wrong source checkout, and missing Codex. Codex uses **read-only** sandbox, ignores user/project config and does not persist an agent rollout; the final report stays in the standard user's LOCALAPPDATA/WhereIsItAI-QA/CodexAudits directory. Inspect and redact before sharing. The launcher does not push, commit, alter other projects, install software or perform GUI input.

Independent read-only review focuses: categories, win64-installer, release-risks. Run them from separate normal-user terminal sessions if desired. These reviews are not substitutes for automated Rust tests or human-independent GUI acceptance. Do not claim agents were launched merely because this documentation exists.

## Lane 3: interactive native Win11 RDP GUI / SQLite QA

Take the exact-SHA hosted Windows native egui preview bundle or isolated Win64 installed QA candidate from **successful GitHub-hosted CI** (not untrusted uploads). Check the Actions run ID, source SHA and SHA-256/provenance before executing any unsigned software.

For the side-by-side installed candidate, follow:

- tools/qa/windows11-egui/Verify-Win64EguiCandidate.ps1
- docs/qa/WIN11_WIN64_EGUI_MANUAL_CHECKLIST_20261121.md
- docs/qa/EGUI_CATEGORY_P0_CHECKPOINT_20261121.md (in PR #73 while draft)

For preview mode use tools/qa/windows10-egui/Invoke-WhereIsItEguiQA.ps1 (Win11 supported) under the **interactive** standard user. The script creates synthetic volumes/catalog only, checks SQLite/search/exports and opens a visible egui window; the -ManualReview option requires actual observations. The automated hosted Windows CI does NOT establish GUI parity, physical USB or 3M real CTF performance.

Only provide **redacted aggregate findings** to GitHub: source SHA, workflow run ID, installed EXE hash, OS build/architecture, per-scenario PASS/FAIL/BLOCKED, reviewed screenshot atlas and no personal paths. No private CTF bytes or metadata in public artifacts.

## Lane 4: future dedicated runner for agent development (not implemented)

To run Codex unattended on the VM, create a **separate non-admin runner identity** and fresh isolated workspaces with no secrets, review/approval gates, strict paths and no access to other projects' services. Only after that can a separate trusted manual workflow run codex exec under this restricted identity. The existing administrator runner must remain manual inventory-only. The host is not an isolation boundary; do not allow arbitrary public PR code or unreviewed agent commands.

## Current release gates

The Win64 egui installed QA smoke passed in GitHub-hosted CI and PR #72 is in main. PR #73 exact source SHA 7bfdfb15 passed Linux/Windows/macOS ARM+Intel, offline, integration and upgrade workflows; independent interactive Win11 categories still **NOT_RUN**, not a blocker that may be silently waived for stable. Stable 1.0 remains NO-GO pending GUI/USB, changed CTF in original app, real 3M p95, signed installers, and other master-plan gates.
