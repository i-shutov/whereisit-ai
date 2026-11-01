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
        self.assertIn("fn disk_locator_text(", WIN)
        self.assertIn("entry: Option<&Value>", WIN)
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
        self.assertIn('Button("Search all disks") { model.searchAcrossAllDisks() }', MAC)
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

    def test_live_os_physical_pairing_is_explicit_and_independent(self):
        identity = (ROOT / "crates/atlas-service/src/physical_disks.rs").read_text(encoding="utf-8")
        self.assertIn('"physical_status" => physical_disks::status(&v)?', SERVICE)
        self.assertIn('"physical_pair" => physical_disks::pair(&v)?', SERVICE)
        self.assertIn('"physical_unpair" => physical_disks::unpair(&v)?', SERVICE)
        self.assertIn("CREATE TABLE IF NOT EXISTS physical_bindings", identity)
        self.assertIn("UNIQUE (catalog_key, platform, physical_id)", identity)
        self.assertIn('return "unverified"', identity)
        self.assertIn('"identity_conflict"', identity)
        self.assertIn("GetLogicalDriveStringsW", WIN)
        self.assertIn("GetVolumeNameForVolumeMountPointW", WIN)
        self.assertIn("GetVolumeInformationW", WIN)
        self.assertIn("self.refresh_physical_presence();", WIN)
        self.assertIn("PHYSICAL_PAIR => self.pair_selected_physical_disk()", WIN)
        self.assertIn("volumeUUIDStringKey", MAC)
        self.assertIn("mountedVolumeURLs", MAC)
        self.assertIn("warning.runModal()", MAC)
        self.assertIn("physicalStates = [:]", MAC)
        self.assertIn("physicalArguments(freshProbe, probeOK: true)", MAC)
        self.assertIn('Button("Refresh OS mounts")', MAC)
        self.assertIn('Button("Pair selected catalog disk…")', MAC)

    def test_search_defaults_to_all_disks_and_local_scope_is_explicit(self):
        self.assertIn("FIND | FIND_DISK | DUPLICATES =>", WIN)
        self.assertIn("if command == FIND || command == DUPLICATES", WIN)
        self.assertIn('FIND_DISK, "&This disk"', WIN)
        self.assertIn("Global search across all catalogued disks", WIN)
        self.assertIn("func searchInSelectedDisk()", MAC)
        self.assertIn("var resultScopeCaption: String", MAC)
        self.assertIn('Button("Search all disks") { model.searchAcrossAllDisks() }', MAC)
        self.assertIn('Button("This disk") { model.searchInSelectedDisk() }', MAC)
        self.assertIn("METADATA DUPLICATES • ALL CATALOGUED DISKS", MAC)

    def test_pairing_picker_rejects_colliding_live_volume_uuids(self):
        self.assertIn("fn unique_pairable_mounts(", WIN)
        self.assertIn("cloned_and_multi_mounted_identity_is_not_a_pairing_choice", WIN)
        self.assertIn("for candidate in candidates {", WIN)
        self.assertIn("var pairablePhysicalVolumes: [LivePhysicalVolume]", MAC)
        self.assertIn("ForEach(model.pairablePhysicalVolumes)", MAC)
        self.assertIn("Duplicate OS volume UUID: ambiguous mounts cannot be paired.", MAC)
        self.assertIn("observed_ids", (ROOT / "crates/atlas-service/src/physical_disks.rs").read_text(encoding="utf-8"))

    def test_register_filter_is_view_only_and_preserves_real_disk_indices(self):
        self.assertIn("fn filtered_disk_indices(", WIN)
        self.assertIn("fn filtered_disk_index_at_row(", WIN)
        self.assertIn("disk_register_filter_tests", WIN)
        self.assertIn("filtered_disk_index_at_row(&self.visible_disk_indices, index)", WIN)
        self.assertIn("self.populate_physical_disk_list();", WIN)
        self.assertIn("DISK_FILTER if notification == EN_CHANGE as usize", WIN)
        self.assertIn("GetFocus() == (*app).disk_filter", WIN)
        self.assertIn("message.wParam == b'L' as usize", WIN)
        self.assertIn("var visibleVolumes: [Volume]", MAC)
        self.assertIn("ForEach(model.visibleVolumes)", MAC)
        self.assertIn("Filter catalog disk register", MAC)
        self.assertIn("model.diskFilter = \"\"", MAC)
        self.assertIn('.keyboardShortcut("l", modifiers: [.command])', MAC)
        self.assertIn("View-only filtering: never change volumeID", MAC)

    def test_native_locator_panels_can_resize_or_collapse_without_changing_scope(self):
        self.assertIn("fn explorer_geometry_for_width(", WIN)
        self.assertIn("fn result_column_widths(", WIN)
        self.assertIn("fn locator_splitter_hit_test(", WIN)
        self.assertIn("WM_LBUTTONDOWN =>", WIN)
        self.assertIn("WM_MOUSEMOVE if app.dragging_locator =>", WIN)
        self.assertIn("WM_CAPTURECHANGED =>", WIN)
        self.assertIn("TOGGLE_LOCATOR =>", WIN)
        self.assertIn("GetSysColorBrush(COLOR_3DSHADOW)", WIN)
        self.assertIn("locator_splitter_tests", WIN)
        self.assertIn("message.wParam == b'I' as usize", WIN)
        self.assertIn("@State private var locatorVisible = true", MAC)
        self.assertIn('Button(locatorVisible ? "Hide locator" : "Show locator")', MAC)
        self.assertIn('if locatorVisible {\n                    DiskLocatorInspector(model: model)', MAC)
        self.assertIn('.keyboardShortcut("i", modifiers: [.command, .shift])', MAC)
        self.assertIn("minWidth: locatorVisible ? 1100 : 880", MAC)
        self.assertNotIn("self.volume = None;", WIN.split("TOGGLE_LOCATOR =>", 1)[1].split("PHYSICAL_CHECK =>", 1)[0])
        self.assertNotIn("model.volumeID =", MAC.split('Button(locatorVisible ? "Hide locator" : "Show locator")', 1)[1].split(".font(.caption)", 1)[0])

    def test_no_unverified_physical_drive_claims_in_locator(self):
        self.assertIn("Physical connection: NOT VERIFIED", WIN)
        self.assertIn("Physical connection not verified", MAC)
        self.assertIn("not content-verified", WIN)
        self.assertIn("not verified by content", MAC)
        self.assertIn("Linked CTF — read-only", MAC)
        self.assertIn("source read-only; SHA generation pinned", WIN)


if __name__ == "__main__":
    unittest.main()
