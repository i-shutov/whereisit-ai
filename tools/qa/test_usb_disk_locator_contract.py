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
SERVICE = (ROOT / "crates/atlas-service/src/lib.rs").read_text(encoding="utf-8")
COPIES = (ROOT / "crates/atlas-service/src/copy_candidates.rs").read_text(encoding="utf-8")


class DiskLocatorWiringTests(unittest.TestCase):
    def test_windows_selection_is_driven_by_current_page(self):
        self.assertIn("fn disk_locator_text(entry: Option<&Value>", WIN)
        self.assertIn("header.code == LVN_ITEMCHANGED", WIN)
        self.assertIn("app.update_disk_locator();", WIN)
        self.assertIn("self.entries.get(index as usize)", WIN)
        self.assertIn("ES_READONLY as u32", WIN)
        self.assertIn("if window_width >= 1176", WIN)
        self.assertIn("LVM_SETCOLUMNWIDTH", WIN)
        self.assertIn("explorer_geometry_tests", WIN)
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

    def test_search_keyboard_and_all_disk_shortcuts_are_explicit(self):
        self.assertIn("GetKeyState(VK_CONTROL as i32) < 0", WIN)
        self.assertIn("SetFocus((*app).search);", WIN)
        self.assertIn("SendMessageW((*app).search, EM_SETSEL, 0, -1)", WIN)
        self.assertIn("@FocusState private var searchFieldFocused: Bool", MAC)
        self.assertIn(".focused($searchFieldFocused)", MAC)
        self.assertIn('Button("All disks") { model.searchAcrossAllDisks() }', MAC)
        self.assertIn('.keyboardShortcut("f", modifiers: [.command])', MAC)

    def test_selected_file_copy_navigation_is_native_and_source_pinned(self):
        self.assertIn("FILE_COPIES => self.show_copies_of_selected()", WIN)
        self.assertIn('self.linked_request("ctf_copy_candidates_pinned")', WIN)
        self.assertIn('"op":"copy_candidates"', WIN)
        self.assertIn("message.wParam == b'D' as usize", WIN)
        self.assertIn('Button("Find copies on other disks") { model.copiesOfSelected() }', MAC)
        self.assertIn('args["op"] = "ctf_copy_candidates_pinned"', MAC)
        self.assertIn('args["op"] = "copy_candidates"', MAC)
        self.assertIn('copySourceID = nil; copySourceName = nil; copyReturn = nil', MAC)
        self.assertIn('"ctf_copy_candidates_pinned" => copy_candidates::linked(&v)?', SERVICE)
        self.assertIn('"copy_candidates" => copy_candidates::page(', SERVICE)
        self.assertIn('ctf_mirror::read_pinned(request,', COPIES)
        self.assertIn('name_norm=?1 AND size=?2', COPIES)
        self.assertIn('"verified": false', COPIES)

    def test_no_unverified_physical_drive_claims_in_locator(self):
        self.assertIn("Physical connection: NOT VERIFIED", WIN)
        self.assertIn("Physical connection not verified", MAC)
        self.assertIn("not content-verified", WIN)
        self.assertIn("not verified by content", MAC)
        self.assertIn("Linked CTF — read-only", MAC)
        self.assertIn("source read-only; SHA generation pinned", WIN)


if __name__ == "__main__":
    unittest.main()
