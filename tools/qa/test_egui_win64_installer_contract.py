"""Win64 installed native egui QA contract; no real user data or privileged runners."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


class NativeWin64InstalledCandidate(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.iss = (ROOT / "installers/windows/WhereIsItAI-EguiWin64QA.iss").read_text(encoding="utf-8")
        cls.build = (ROOT / "installers/windows/build-egui-win64-qa.ps1").read_text(encoding="utf-8")
        cls.smoke = (ROOT / "installers/windows/smoke-egui-win64-qa.ps1").read_text(encoding="utf-8")
        cls.ci = (ROOT / ".github/workflows/build.yml").read_text(encoding="utf-8")
        cls.legacy = (ROOT / "installers/windows/WhereIsItAI.iss").read_text(encoding="utf-8")

    def test_installer_is_distinct_native_egui_win64_only(self):
        self.assertIn("ArchitecturesAllowed=x64compatible", self.iss)
        self.assertIn("ArchitecturesInstallIn64BitMode=x64compatible", self.iss)
        self.assertIn("PrivilegesRequired=lowest", self.iss)
        self.assertIn("atlas-desktop-egui.exe", self.iss)
        self.assertNotIn('Source: "{#BuildDir}/atlas-windows.exe"', self.iss)
        self.assertIn("whereisit-egui-win64.exe", self.iss)
        self.assertIn("THIRD_PARTY_NOTICES.md", self.iss)
        self.assertIn("licenses/*", self.iss)
        self.assertIn("WhereIsItAI-egui-win64-QA", self.iss)
        self.assertNotIn("postinstall", self.iss.lower())
        self.assertNotIn("desktopicon", self.iss.lower())
        self.assertNotIn("ChangesAssociations=yes", self.iss)

        def attr(contents, key):
            prefix = key + "="
            return next(line.strip() for line in contents.splitlines()
                        if line.strip().startswith(prefix))[len(prefix):]
        self.assertNotEqual(attr(self.iss, "AppId"), attr(self.legacy, "AppId"))
        self.assertNotEqual(attr(self.iss, "DefaultDirName"),
                            attr(self.legacy, "DefaultDirName"))

    def test_fail_closed_actual_amd64_installed_hash_and_external_sqlite(self):
        self.assertIn("0x8664", self.smoke)
        self.assertIn("Get-PeMachine", self.smoke)
        self.assertIn("atlas-desktop-egui.exe", self.smoke)
        self.assertIn("atlas-windows.exe", self.smoke)
        self.assertIn("SHA256", self.smoke)
        self.assertIn("whereisit-egui-win64.exe", self.smoke)
        self.assertIn("whereisit-ai-cli.exe", self.smoke)
        self.assertIn("ConvertFrom-Json", self.smoke)
        self.assertIn("Refusing to reuse existing catalog path", self.smoke)
        self.assertIn("Uninstaller changed external catalog bytes", self.smoke)
        self.assertNotIn("Remove-Item", self.smoke)
        self.assertNotIn("Invoke-WebRequest", self.smoke)
        self.assertIn("Refusing QA installer overwrite", self.build)

    def test_compiled_only_on_github_hosted_windows_and_not_stable(self):
        self.assertIn("runs-on: windows-2022", self.ci)
        self.assertIn("build-egui-win64-qa.ps1", self.ci)
        self.assertIn("smoke-egui-win64-qa.ps1", self.ci)
        self.assertIn("WhereIsItAI-egui-Win64-installed-QA-unsigned", self.ci)
        self.assertIn("signing = 'unsigned'", self.ci)
        self.assertIn("stable_ready = $false", self.ci)
        self.assertIn("installer_sha256", self.ci)
        self.assertIn("egui_sha256", self.ci)
        self.assertNotIn("self-hosted", self.ci)
        self.assertIn("atlas-windows.exe", self.legacy)
        cargo = (ROOT / "apps/desktop-egui/Cargo.toml").read_text(encoding="utf-8")
        isolation = (ROOT / "apps/desktop-egui/src/qa_isolation.rs").read_text(encoding="utf-8")
        ui = (ROOT / "apps/desktop-egui/src/ui.rs").read_text(encoding="utf-8")
        main = (ROOT / "apps/desktop-egui/src/main.rs").read_text(encoding="utf-8")
        self.assertIn("qa-isolated = []", cargo)
        self.assertIn("qa-isolated", self.ci)
        self.assertIn("Restore default preview GUI binary", self.ci)
        self.assertIn("WHEREISIT_USER_STORE", ui)
        self.assertIn("crate::qa_isolation::user_store_path()", ui)
        self.assertIn("persistence_path: Some(qa_settings_path)", main)
        self.assertIn("WhereIsIt AI egui Win64 QA", isolation)
        self.assertIn("physical-volume-bindings.sqlite", isolation)

        preflight = (ROOT / "tools/qa/windows11-egui/Verify-Win64EguiCandidate.ps1").read_text(encoding="utf-8")
        self.assertIn("ExpectedCommit", preflight)
        self.assertIn("ExpectedRunId", preflight)
        self.assertIn("SHA256SUMS.txt", preflight)
        self.assertIn("installer_sha256", preflight)
        self.assertIn("WIN64_EGUI_QA_PREFLIGHT_PASS", preflight)
        self.assertNotIn("Start-Process", preflight)
        self.assertNotIn("Invoke-WebRequest", preflight)
        self.assertIn("Verify-Win64EguiCandidate.ps1", self.ci)



if __name__ == "__main__":
    unittest.main()
