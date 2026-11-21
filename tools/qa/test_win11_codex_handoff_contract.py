"""Guard trusted-main Administrator runner/Codex handoff from unsafe PR execution.

Static tests do not establish real Windows 11 execution or GUI acceptance.
"""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


class Win11CodexRunnerContract(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.smoke = (ROOT / ".github/workflows/win11-runner-smoke.yml").read_text(encoding="utf-8")
        cls.script = (ROOT / "tools/qa/windows11-egui/Invoke-Win11CodexReadOnlyAudit.ps1").read_text(encoding="utf-8")
        cls.ci = (ROOT / ".github/workflows/build.yml").read_text(encoding="utf-8")

    def test_admin_runner_is_manual_trusted_main_only(self):
        self.assertRegex(self.smoke, r"(?m)^on:\s*\n\s+workflow_dispatch:")
        self.assertRegex(self.smoke, r"(?m)^\s+runs-on: \[self-hosted, Windows, X64, win11-whereisit\]")
        self.assertIn("github.ref == 'refs/heads/main'", self.smoke)
        self.assertIn("github.repository == 'i-shutov/whereisit-ai'", self.smoke)
        self.assertNotRegex(self.smoke, r"(?m)^\s+pull_request(?:_target)?:")
        self.assertNotIn("actions/checkout@", self.smoke)
        self.assertNotIn("codex exec", self.smoke.lower())
        self.assertIn("permissions:\n  contents: read", self.smoke)
        self.assertIn("CODEX_EXECUTED: false", self.smoke)
        self.assertIn("CODEX_CLI_VISIBLE_TO_SERVICE", self.smoke)

    def test_extensionless_runner_script_uses_safe_process_shell(self):
        # Run #44885465184: Restricted policy blocked builtin powershell.
        # Run #44886507387: custom -File {0} rejected extensionless temp file.
        # The reviewed main-only shell must read temp script as text.
        shells = re.findall(r"(?m)^\s+shell:\s+(.+)$", self.smoke)
        expected = "powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \"& ([scriptblock]::Create([IO.File]::ReadAllText('{0}')))\""
        self.assertEqual(len(shells), 3)
        self.assertTrue(all(s.strip() == expected for s in shells), shells)
        self.assertNotIn("-File {0}", self.smoke)
        self.assertNotIn("Set-ExecutionPolicy", self.smoke)
        self.assertNotIn("Set-ItemProperty", self.smoke)
        self.assertIn("Win11 shell template hosted probe", self.ci)
        self.assertIn(expected, self.ci)

    def test_public_logs_do_not_disclose_private_accounts_or_paths(self):
        self.assertNotIn('Write-Host "RUNNER_HOST: $env:COMPUTERNAME"', self.smoke)
        self.assertNotIn('Write-Host "SERVICE_ACCOUNT: $actualUser"', self.smoke)
        self.assertNotIn("$($c.Source)", self.smoke)
        self.assertIn("RUNNER_HOST_MATCH: true", self.smoke)
        self.assertIn("SERVICE_ACCOUNT_MATCH: true", self.smoke)

    def test_local_codex_allows_admin_interactive_but_rejects_service(self):
        self.assertIn("Administrator", self.script)
        self.assertIn("UserInteractive", self.script)
        self.assertIn("ACCOUNT_HAS_ADMIN_TOKEN", self.script)
        self.assertNotIn("Do not run Codex under an elevated/administrator account.", self.script)
        self.assertIn("RUNNER_NAME", self.script)
        self.assertIn("ExpectedSha", self.script)
        self.assertIn("git -C $root rev-parse HEAD", self.script)
        self.assertIn("git -C $root status --porcelain", self.script)
        self.assertIn("if (-not $Execute)", self.script)
        self.assertIn("--sandbox read-only", self.script)
        self.assertIn("--ignore-user-config", self.script)
        self.assertIn("--ephemeral", self.script)
        self.assertIn("REPORT_PRIVATE_LOCAL_ONLY", self.script)
        for forbidden in ("--yolo", "danger-full-access", "--sandbox workspace-write",
                          "git push", "git clone", "Stop-Service", "Invoke-WebRequest"):
            self.assertNotIn(forbidden, self.script)

    def test_ci_validates_contract_and_only_parses_powershell(self):
        self.assertIn("test_win11_codex_handoff_contract.py", self.ci)
        self.assertIn("Parse Win11 Codex read-only launcher", self.ci)
        self.assertIn("Language.Parser]::ParseFile", self.ci)
        self.assertNotIn("Invoke-Win11CodexReadOnlyAudit.ps1 -Execute", self.ci)


if __name__ == "__main__":
    unittest.main()
