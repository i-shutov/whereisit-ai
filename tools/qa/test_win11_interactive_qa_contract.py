"""Static source guard for interactive Windows 11 Administrator QA handoff.

Does NOT execute unsigned binaries, CTF, USB, GUI, or a privileged runner.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


class Win11InteractiveQAHandoff(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.ps = (ROOT / "tools/qa/windows11-egui/Invoke-VerifiedWin11EguiRdpQA.ps1").read_text(encoding="utf-8")
        cls.ci = (ROOT / ".github/workflows/win11-rdp-qa-launcher-check.yml").read_text(encoding="utf-8")

    def test_interactive_admin_and_provenance_guards(self):
        ps = self.ps
        for fragment in (
            "#Requires -Version 5.1", "RUNNER_NAME", "UserInteractive",
            "Is64BitOperatingSystem", "ExpectedCommit", "provenance-windows-x64.json",
            "SHA256-windows-x64.txt", "QA_MANIFEST.json", "Get-FileHash",
            "gui_sha256", "cli_sha256", "Expand-Archive", "0xEF", "0xBB", "0xBF",
            "-DiskCount $DiskCount", "-ManualReview", "-CategoryChecks",
            "-CaptureScreenshot", "hardware_usb_acceptance",
            "WIN11_QA_ARTIFACT_INTEGRITY_PASS",
            "WIN11_INTERACTIVE_EVIDENCE_COLLECTED",
        ):
            self.assertIn(fragment, ps)
        self.assertNotIn("non-admin", ps)
        for forbidden in (
            "Invoke-WebRequest", "Start-BitsTransfer", "Remove-Item", "Stop-Service",
            "New-Service", "Set-ExecutionPolicy", "Set-NetFirewallRule", "git clone",
            "git push", "Enter-PSSession", "Enable-PSRemoting",
        ):
            self.assertNotIn(forbidden, ps)

    def test_hosted_ci_may_parse_but_never_launch_gui(self):
        ci = self.ci
        self.assertIn("runs-on: windows-2022", ci)
        self.assertNotIn("self-hosted", ci)
        self.assertIn("Language.Parser]::ParseFile", ci)
        self.assertIn("test_win11_interactive_qa_contract.py", ci)
        self.assertNotIn("Invoke-VerifiedWin11EguiRdpQA.ps1 -ArtifactDirectory", ci)
        self.assertIn("contents: read", ci)


if __name__ == "__main__":
    unittest.main()
