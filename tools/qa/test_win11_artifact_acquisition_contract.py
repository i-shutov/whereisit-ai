"""Contract for Win11 Codex self-provisioning of exact GitHub-hosted Win64 artifacts.

Static only: does not authenticate to GitHub or execute unsigned programs.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


class Win11AgentArtifactAcquisitionContract(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.script = (ROOT / "tools/qa/windows11-egui/Get-VerifiedWin64QABundles.ps1").read_text(encoding="utf-8")
        cls.docs = (ROOT / "docs/qa/WIN11_CODEX_GET_PROGRAM_20261121.md").read_text(encoding="utf-8")
        cls.task = (ROOT / "docs/qa/WIN11_CODEX_REAL_CTF_ORIGINAL_ACCEPTANCE_20261121.md").read_text(encoding="utf-8")

    def test_exact_source_and_download_contract(self):
        for marker in ("i-shutov/whereisit-ai", "gh.exe", "run view", "headSha",
                       "conclusion", "workflowName", "completed", "success",
                       "native CI and installer candidates", "run download",
                       "WhereIsItAI-egui-qa-windows-x64",
                       "WhereIsItAI-egui-Win64-installed-QA-unsigned",
                       "provenance-windows-x64.json", "SHA256-windows-x64.txt",
                       "Get-FileHash", "Verify-Win64EguiCandidate.ps1",
                       "ExpectedCommit", "ExpectedRunId",
                       "WIN64_QA_BUNDLES_DOWNLOADED_VERIFIED"):
            self.assertIn(marker, self.script)

    def test_fail_closed_no_gui_or_privacy_violations(self):
        for marker in ("Refusing to overwrite", "UserInteractive", "RUNNER_NAME",
                       "GUI_EXECUTED: false", "INSTALLER_EXECUTED: false",
                       "REAL_CTF_TOUCHED: false"):
            self.assertIn(marker, self.script)
        for forbidden in ("Invoke-WebRequest", "Set-ExecutionPolicy",
                          "Start-Process", "Remove-Item", "git push",
                          "GH_TOKEN", "curl.exe", "--yolo"):
            self.assertNotIn(forbidden, self.script)

    def test_task_provisions_program_without_user_upload(self):
        self.assertIn("44891158178", self.docs)
        self.assertIn("40c6b695958e49cbdad78affa314b139dd28d61b", self.docs)
        self.assertIn("gh auth login --web", self.docs)
        self.assertIn("Invoke-VerifiedWin11EguiRdpQA.ps1", self.docs)
        self.assertIn("3.54.1", self.docs)
        self.assertIn("оригинальная WhereIsIt", self.docs)
        self.assertIn("WIN11_CODEX_GET_PROGRAM_20261121.md", self.task)


if __name__ == "__main__":
    unittest.main()
