"""Source-wiring checks for the native offline-disk locator shell.

These checks supplement compiled Windows unit tests and macOS AppKit/SwiftUI
builds. They do NOT claim manual native GUI acceptance or physical USB presence.
No private CTF/SQLite files are loaded.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
WIN = (ROOT / "apps/windows/src/native.rs").read_text(encoding="utf-8")
MAC = (ROOT / "apps/macos/Sources/AtlasCatalog/AtlasCatalog.swift").read_text(encoding="utf-8")


class DiskLocatorWiringTests(unittest.TestCase):
    def test_windows_selection_is_driven_by_current_page(self):
        self.assertIn("fn disk_locator_text(entry: Option<&Value>", WIN)
        self.assertIn("header.code == LVN_ITEMCHANGED", WIN)
        self.assertIn("app.update_disk_locator();", WIN)
        self.assertIn("self.entries.get(index as usize)", WIN)
        self.assertIn("ES_READONLY as u32", WIN)
        self.assertIn("if w >= 1120", WIN)
        self.assertIn("SW_HIDE", WIN)
        self.assertIn("disk_locator_tests", WIN)

    def test_macos_selected_row_and_navigation_are_wired(self):
        self.assertIn("DiskLocatorInspector(model: model)", MAC)
        self.assertIn("if let entry = model.selected", MAC)
        self.assertIn("model.browseDisk(entry.volume_id)", MAC)
        self.assertIn("model.searchAcrossAllDisks()", MAC)
        self.assertIn("NSPasteboard.general.setString", MAC)
        self.assertIn("model.volumeLabel(entry.volume_id)", MAC)
        self.assertIn("Text(entry.relative_path)", MAC)

    def test_no_unverified_physical_drive_claims_in_locator(self):
        self.assertIn("Physical connection: NOT VERIFIED", WIN)
        self.assertIn("Physical connection not verified", MAC)
        self.assertIn("not content-verified", WIN)
        self.assertIn("not verified by content", MAC)
        self.assertIn("Linked CTF — read-only", MAC)
        self.assertIn("source read-only; SHA generation pinned", WIN)


if __name__ == "__main__":
    unittest.main()
