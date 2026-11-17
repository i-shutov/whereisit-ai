"""Static safeguards for side-by-side native egui QA packaging."""
from pathlib import Path
import unittest
ROOT = Path(__file__).resolve().parents[2]
class EguiQaPreviewPackagingContract(unittest.TestCase):
    def test_preview_is_isolated_from_stable_installers(self):
        mac = (ROOT / 'apps/desktop-egui/package-preview-macos.sh').read_text(encoding='utf-8')
        win = (ROOT / 'apps/desktop-egui/package-preview-windows.ps1').read_text(encoding='utf-8')
        actions = (ROOT / '.github/workflows/build.yml').read_text(encoding='utf-8')
        self.assertIn('qa-egui', mac)
        self.assertIn('qa-egui', win)
        self.assertIn('ai.whereisit.egui.preview', mac)
        self.assertIn('ad-hoc, not notarized', mac)
        self.assertIn('unsigned', win.lower())
        self.assertIn('SHA256', mac)
        self.assertIn('SHA256', win)
        self.assertNotIn('notarytool', mac.lower())
        self.assertIn('WhereIsItAI-egui-qa-windows-x64', actions)
        self.assertIn('WhereIsItAI-egui-qa-', actions)
        self.assertIn('WhereIsItAI-Windows-x64-unsigned-candidate', actions)
        self.assertIn('WhereIsItAI-Egui-QA', actions)
        self.assertIn('Invoke-WhereIsItEguiQA.ps1', win)
        self.assertIn('whereisit-ai-cli.exe', win)
        self.assertIn('QA_MANIFEST.json', win)
if __name__ == '__main__':
    unittest.main()
