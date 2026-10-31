"""Source-wiring regression, complementary to compiled Rust/Swift and service tests.

This is NOT a physical GUI pass. It prevents stale disabled controls from making
already-tested generation-pinned duplicate/JSON export APIs unreachable.
"""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


class NativeExportWiring(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.win = (ROOT/'apps/windows/src/native.rs').read_text()
        cls.mac = (ROOT/'apps/macos/Sources/AtlasCatalog/AtlasCatalog.swift').read_text()

    def test_windows_exposes_json_without_allowing_source_ctf_writes(self):
        self.assertIn('JSON — current branch/query', self.win)
        self.assertIn('3 => "export_json"', self.win)
        self.assertIn('3 => "json"', self.win)
        self.assertIn('matches!(index, 0 | 1 | 3)', self.win)
        self.assertGreaterEqual(self.win.count('linked_export_allowed('), 4)
        self.assertNotIn('Linked duplicate candidates cannot be exported', self.win)
        self.assertNotRegex(self.win, r'command == EXPORT\s*&& !self\.duplicates')

    def test_windows_preserves_generation_pinned_full_duplicate_dispatch(self):
        self.assertIn('self.linked_request("ctf_export_pinned")', self.win)
        self.assertRegex(self.win, r'linked\["mode"\] = json!\(if self\.duplicates\s*\{\s*"duplicates"')
        self.assertIn('linked["duplicate_mode"] = json!(self.applied_duplicate_mode)', self.win)
        self.assertRegex(self.win, r'if !self\.duplicates\s*\{\s*linked\["parent_id"\]')

    def test_macos_duplicate_button_is_enabled_and_json_is_selectable(self):
        self.assertIn('Text("JSON • current scope").tag("json")', self.mac)
        self.assertNotIn('model.exportFormat == "ctf" || model.showingDuplicates', self.mac)
        self.assertIn('["csv", "parquet", "json"].contains(exportFormat)', self.mac)
        self.assertIn('"op": "ctf_export_pinned"', self.mac)

    def test_macos_save_panel_captures_scope_not_mutable_post_dialog_state(self):
        self.assertIn('let selectedQuery = appliedQuery', self.mac)
        self.assertIn('let selectedDuplicateMode = appliedDuplicateMode', self.mac)
        self.assertIn('args["mode"] = selectedMode', self.mac)
        self.assertIn('args["query"] = selectedQuery', self.mac)
        self.assertIn('args["duplicate_mode"] = selectedDuplicateMode', self.mac)
        self.assertIn('if selectedMode != "duplicates"', self.mac)
        self.assertIn('self.linkedGeneration == originalGeneration', self.mac)


if __name__ == '__main__':
    unittest.main()
