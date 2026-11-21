# WhereIsIt 1.0 — Win11 W1 runner acceptance checkpoint

**Recorded 2026-11-21.** Observed source: trusted `main` commit `9ecd3f147a7ac7214c47369f766f7c8edfd14b15`, manually dispatched GitHub Actions [run #44888299971](https://github.com/i-shutov/whereisit-ai/actions/runs/44888299971), job `windows11-native`: **PASS**.

## Observed evidence

| Check | Observed status | Scope |
|---|---|---|
| Self-hosted runner accepted exact manual main job | PASS | Runner label `win11-whereisit` matched, no PR checkout |
| Windows version and identity preflight | PASS | Windows 11 Pro x64, build 22631; approved Administrator service token |
| PowerShell host | PASS | Windows PowerShell 5.1 ran trusted checks |
| All runner smoke steps | PASS | Host validation and non-blocking toolchain availability survey |
| Codex CLI visible in runner service | NOT_RUN | Old SHA did not include later PR #74 Codex detection step |
| Codex CLI in interactive Administrator session | NOT_RUN | Different execution context; user reported installation, not independently tested |
| Interactive native egui GUI in RDP | NOT_RUN | Runner service runs headless and cannot prove GUI |
| Real USB unplug/replug, CTF modified output, 3M p95 | NOT_RUN | Independent stable-release gates |

## Toolchain observation

At the runner service command PATH, `cargo.exe`, `rustc.exe`, `git.exe`, `sqlite3.exe` and `cl.exe` were not found. A `python.exe` command was found under the WindowsApps app-alias area, **not** proof that a usable Python interpreter is installed. This does not establish absence of any tool in the interactive Administrator session, and is not a runner-smoke failure. A separate read-only toolchain preflight must inspect known user and machine paths before native builds on this host.

## Boundaries

Do not publish machine address, service login or full private paths. This checkpoint is **W1_RUNNER=PASS; W1_INTERACTIVE_GUI=NOT_RUN**. It establishes no hardware/GUI/signing acceptance. User-authorized Win11 RDP, PowerShell, Codex and egui QA run under Administrator as explicitly stated in `AGENTS.md`; no standard user account is required. A new trusted-main manual smoke after PR #74 can check the additional `CODEX_CLI_VISIBLE_TO_SERVICE` marker without running Codex.

Stable 1.0 remains **NO-GO**; see `release/ACCEPTANCE.json` and the W0–W7 action plan.
