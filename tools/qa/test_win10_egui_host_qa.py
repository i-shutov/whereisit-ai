"""Safety and completeness of Win10 Pro RDP QA bundle and runner."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]

class Win10EguiQaHostContract(unittest.TestCase):
    def test_host_runner_is_offline_and_non_destructive(self):
        text = (ROOT / "tools/qa/windows10-egui/Invoke-WhereIsItEguiQA.ps1").read_text(encoding="utf-8")
        self.assertIn("#Requires -Version 5.1", text)
        self.assertIn("DiskCount = 60", text)
        self.assertIn("SkipGui", text)
        self.assertIn("ManualReview", text)
        self.assertIn("CaptureScreenshot", text)
        self.assertIn("Get-FileHash", text)
        self.assertIn("QA_MANIFEST.json", text)
        self.assertIn("Join-Path $script:RunFolder 'synthetic-media'", text)
        for op in ("scan", "search_fast", "export_csv", "export_json", "export_parquet",
                   "validate_catalog", "physical_shelf_label_set", "physical_status"):
            self.assertIn(op, text)
        self.assertIn("Start-Process -FilePath $gui", text)
        self.assertIn("MainWindowHandle", text)
        self.assertIn("CloseMainWindow()", text)
        self.assertIn("Stop-Process -Id $script:PreviewProcess.Id", text)
        self.assertIn("qa_report.json", text)
        self.assertIn("qa_report.md", text)
        self.assertIn("hardware_usb_acceptance = 'NOT_RUN'", text)
        for forbidden in ("FlowBridgeExplicitBeta", "FlowBridge-ReleaseQA",
                          "Set-NetFirewallRule", "Enable-PSRemoting", "New-Service",
                          "Set-ExecutionPolicy -Scope LocalMachine", "Remove-Item -Recurse"):
            self.assertNotIn(forbidden, text)

    def test_windows_ci_exercises_script_without_claiming_rdp_qa(self):
        workflow = (ROOT / ".github/workflows/build.yml").read_text(encoding="utf-8")
        bundler = (ROOT / "apps/desktop-egui/package-preview-windows.ps1").read_text(encoding="utf-8")
        self.assertIn("test_win10_egui_host_qa.py", workflow)
        self.assertIn("Windows runner", workflow)
        self.assertIn("-DiskCount 6 -SkipGui", workflow)
        self.assertIn("WhereIsItAI-egui-qa-windows-synthetic-evidence", workflow)
        self.assertIn("Invoke-WhereIsItEguiQA.ps1", bundler)
        self.assertIn("QA_MANIFEST.json", bundler)
        self.assertIn("whereisit-ai-cli.exe", bundler)
        self.assertNotIn("FlowBridge", bundler)

if __name__ == "__main__":
    unittest.main()
