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
PHYSICAL = (ROOT / "crates/atlas-service/src/physical_disks.rs").read_text(encoding="utf-8")


class DiskLocatorWiringTests(unittest.TestCase):
    def test_ux31_context_actions_use_current_row_and_preserve_source_checks(self):
        self.assertIn("fn result_context_actions(", WIN)
        self.assertIn("current_selected_result_is_authorized_by_exact_row_and_catalog_disk_id", WIN)
        self.assertIn("WM_CONTEXTMENU if wp == app.files as usize", WIN)
        self.assertIn("SendMessageW(app.files, LVM_HITTEST", WIN)
        self.assertIn("if !page_ready || page_awaiting", WIN)
        self.assertIn("app.command(selected_command as usize, 0)", WIN)
        self.assertIn("COPY_LOCATOR", WIN)
        self.assertIn("COPY_DISK_CARD", WIN)
        self.assertIn('Menu("File actions")', MAC)
        self.assertIn('Button("Folder") { model.browseSelectedFileFolder() }', MAC)
        self.assertIn('Button("Finder") { model.openCurrentOSMountRoot() }', MAC)
        self.assertIn(".contextMenu {", MAC)
        self.assertIn("if model.pageReady, !model.awaitingPage, let row = model.selected", MAC)
        self.assertIn('Button("Find name + size candidates on other disks")', MAC)
        self.assertIn("model.copySelectedMetadata(", MAC)
        self.assertIn("func retrievalNote(for entry: Entry) -> String", MAC)
        self.assertIn("model.retrievalNote(for: row)", MAC)
        self.assertIn("model.retrievalNote(for: entry)", MAC)
        retrieval_note = MAC.split("func retrievalNote(for entry: Entry) -> String {", 1)[1].split(
            "func shelfConflictCaption(", 1
        )[0]
        self.assertNotIn("model.", retrieval_note)
        self.assertIn("self.shelfLabel(entry.volume_id)", retrieval_note)
        self.assertIn("self.volumeLabel(entry.volume_id)", retrieval_note)
        self.assertIn("self.physicalCaption(entry.volume_id)", retrieval_note)
        self.assertIn('Button("Open verified disk in Finder")', MAC)
        self.assertEqual(MAC.count('.accessibilityLabel("Copy current OS volume root after verifying explicit pairing")'), 1)
        # No direct read of live files or unsafe inferred paths in new actions.
        self.assertNotIn("FileManager.default.fileExists", MAC)


    def test_ux30_incomplete_linked_source_never_falls_back_or_writes(self):
        # A source path with no trusted generation is LINKED/READ-ONLY.
        # Both native clients must refuse SQLite search and disk-card fallback.
        self.assertIn("fn incomplete_linked_ctf(", WIN)
        self.assertIn("missing_generation_never_authorizes_local_fallback", WIN)
        self.assertIn("self.linked_ctf.is_some()\n    }", WIN)
        refresh = WIN.split("unsafe fn refresh(&mut self) {", 1)[1].split(
            "unsafe fn load(&mut self)", 1
        )[0]
        self.assertIn("incomplete_linked_ctf(", refresh)
        self.assertIn("self.volumes.clear();", refresh)
        load = WIN.split("unsafe fn load(&mut self) {", 1)[1].split(
            "unsafe fn ", 1
        )[0]
        self.assertIn("Linked CTF generation unavailable; no local SQLite search", load)
        self.assertIn("self.pending_locator_copy = Some((entry_id, self.generation));", WIN)
        self.assertIn('self.call("locator_source", pinned);', WIN)
        self.assertIn('if reply.tag == "locator_source" {', WIN)
        self.assertIn("self.copy_current_locator_selection(entry_id);", WIN)
        self.assertIn("self.entries.get(row as usize).and_then(id) != Some(entry_id)", WIN)
        self.assertIn("var linkedMode: Bool { linkedCTFPath != nil }", MAC)
        self.assertIn("if linkedMode && linkedFields() == nil {", MAC)
        self.assertIn("Linked CTF generation missing; no SQLite catalog fallback.", MAC)
        disk = MAC.split("func copySelectedDiskPickupCard() {", 1)[1].split(
            "func copySelectedMetadata(", 1
        )[0]
        self.assertIn("if linkedMode {", disk)
        self.assertIn("guard let fields = linkedFields() else {", disk)
        self.assertIn("reply?.generation == self.linkedGeneration", disk)

    def test_ux30_selected_file_clipboard_is_generation_and_page_pinned(self):
        self.assertIn("func copySelectedMetadata(", MAC)
        self.assertIn("selectedMetadataClipboardSerial == serial", MAC)
        self.assertIn("self.pageReady, !self.awaitingPage", MAC)
        self.assertIn("self.selected?.id == entryID, self.selected?.volume_id == diskID", MAC)
        self.assertIn('pinned["op"] = "ctf_volumes_pinned"', MAC)
        self.assertIn("reply?.generation == self.linkedGeneration", MAC)
        self.assertIn("model.copySelectedMetadata(model.volumeLabel(entry.volume_id)", MAC)
        self.assertIn("model.copySelectedMetadata(entry.relative_path", MAC)
        self.assertIn("model.copySelectedMetadata(retrievalNote(for: entry)", MAC)
        self.assertNotIn('Button("Copy recorded path") { copy(', MAC)

    def test_ux28_candidate_folder_return_is_exact_and_revalidated(self):
        # Other-disk candidate -> recorded parent -> candidate scope/page ->
        # original global search. Each return uses a fresh linked CTF request.
        self.assertIn("struct CandidateFolderReturn {", WIN)
        self.assertIn("candidate_excursion_restores_exact_source_focus_and_page", WIN)
        self.assertIn("self.candidate_folder_return = Some(snapshot);", WIN)
        self.assertIn("if let Some(context) = self.candidate_folder_return.take()", WIN)
        self.assertIn("self.copy_source = Some(context.source_id);", WIN)
        self.assertIn("self.copy_focus_volume_id = context.focus_volume_id;", WIN)
        self.assertIn("self.offset = context.offset;", WIN)
        self.assertIn("self.load(); // Fresh pinned CTF source SHA", WIN)
        self.assertIn("private struct CandidateFolderReturnContext", MAC)
        self.assertIn("candidateFolderReturn = CandidateFolderReturnContext(", MAC)
        self.assertIn("if let candidate = candidateFolderReturn {", MAC)
        self.assertIn("copySourceID = candidate.sourceID", MAC)
        self.assertIn("copyFocusVolumeID = candidate.focusVolumeID", MAC)
        self.assertIn("offset = candidate.offset; diskFilter = candidate.diskFilter", MAC)
        folder = MAC.split("func browseSelectedFileFolder() {", 1)[1].split(
            "// Literal filename match", 1
        )[0]
        self.assertNotIn("copySourceID == nil", folder)
        self.assertIn('args["op"] = "ctf_copy_candidates_pinned"', MAC)
        self.assertIn('args["op"] = "ctf_children_pinned"', MAC)
        self.assertIn("let token = generation", MAC)
        self.assertIn("func back()", MAC)
        self.assertNotIn("FileManager.default.fileExists", MAC)


    def test_ux22_second_probe_rejects_new_clone_with_same_uuid_different_mount(self):
        # Source-wiring contract complements the Rust status collision tests
        # and compiled macOS client. Two live observations with the same UUID
        # must fail even if only one retains the original mount location.
        copy = MAC.split("if let pending = self.pendingMountCopy {", 1)[1].split(
            "if let pending = self.pendingConnectedCopyFocus {", 1
        )[0]
        self.assertIn(
            "let freshMatch = secondProbe.first(where: { $0.id == verifiedID })", copy
        )
        self.assertIn("secondProbe.filter({ $0.id == verifiedID }).count == 1", copy)
        self.assertIn("freshMatch.currentMount == mounted.currentMount", copy)
        self.assertNotIn("$0.id == verifiedID && $0.currentMount", copy)
        pair = MAC.split("// Reprobe after the modal", 1)[1].split(
            "let scope = physicalScope", 1
        )[0]
        self.assertIn(
            "let freshMatch = freshProbe.first(where: { $0.id == candidate.id })", pair
        )
        self.assertIn("freshProbe.filter({ $0.id == candidate.id }).count == 1", pair)
        self.assertIn("freshMatch.currentMount == candidate.currentMount", pair)


    def test_ux27_recorded_parent_folder_navigation_is_exact_native_and_pinned(self):
        # A plain Ctrl+F handler must NOT intercept Ctrl+Shift+F first.
        focus = WIN.split("// Ctrl+F focuses global search.", 1)[1].split(
            "// Ctrl+E moves", 1
        )[0]
        self.assertIn("GetKeyState(VK_SHIFT as i32) >= 0", focus)
        folder_shortcut = WIN.split("// Ctrl+Shift+F browses", 1)[1].split(
            "// Ctrl+Shift+N", 1
        )[0]
        self.assertIn("GetKeyState(VK_SHIFT as i32) < 0", folder_shortcut)
        self.assertIn("fn recorded_parent_folder_context(", WIN)
        self.assertIn("fn browse_recorded_parent_folder(&mut self)", WIN)
        self.assertIn("recorded_parent_folder_tests", WIN)
        self.assertIn("BROWSE_RECORDED_FOLDER => self.browse_recorded_parent_folder()", WIN)
        self.assertIn("message.wParam == b'F' as usize", WIN)
        self.assertIn("self.copy_return = Some(CopyReturn {", WIN)
        self.assertIn("self.stack.push((Some(parent_id), breadcrumb));", WIN)
        self.assertIn("self.populate_physical_disk_list();", WIN)
        self.assertIn('"ctf_children_pinned"', WIN)
        self.assertIn("self.linked_request(if self.duplicates {", WIN)
        self.assertIn('"op":"children"', WIN)
        self.assertIn("func browseSelectedFileFolder()", MAC)
        self.assertIn("static func recordedParentBreadcrumb(", MAC)
        self.assertIn("volumes.contains(where: { $0.id == entry.volume_id })", MAC)
        self.assertIn("stack.append((parentID, breadcrumb))", MAC)
        self.assertIn("let context = copyReturn", MAC)
        self.assertIn('args["op"] = "ctf_children_pinned"', MAC)
        self.assertIn('args["op"] = "children"', MAC)
        self.assertIn('Button("Folder") { model.browseSelectedFileFolder() }', MAC)
        self.assertIn('Button("Browse recorded containing folder") { model.browseSelectedFileFolder() }', MAC)
        self.assertIn('.keyboardShortcut("f", modifiers: [.command, .shift])', MAC)
        # Do not let the shortcut imply the current USB filesystem has this file.
        self.assertNotIn("FileManager.default.fileExists", MAC)

    def test_ux29_open_verified_native_root_and_refuse_missing_ctf_pin(self):
        self.assertIn("fn verified_mount_root_across_probes(", WIN)
        self.assertIn("open_native_root_refuses_changed_mount_or_cloned_uuid", WIN)
        self.assertIn("OPEN_VERIFIED_OS_ROOT => self.begin_verified_os_root_action(true)", WIN)
        self.assertIn("COPY_VERIFIED_OS_ROOT => self.begin_verified_os_root_action(false)", WIN)
        self.assertIn('self.linked_ctf.is_some()', WIN)
        self.assertIn('self.linked_request("ctf_volumes_pinned")', WIN)
        self.assertIn("ShellExecuteW(", WIN)
        self.assertIn("if !PathBuf::from(&root).is_absolute()", WIN)
        self.assertIn("verified_mount_root_across_probes(", WIN)
        self.assertIn("message.wParam == b'B' as usize", WIN)
        win_action = WIN.split("unsafe fn operate_verified_os_mount_root(", 1)[1].split(
            "// Copy a physical-retrieval instruction", 1
        )[0]
        self.assertIn("windows_mounted_volumes()", win_action)
        self.assertNotIn('entry["relative_path"]', win_action)
        self.assertIn("func openCurrentOSMountRoot()", MAC)
        self.assertIn("func copyCurrentOSMountRoot()", MAC)
        mac_action = MAC.split("private func performCurrentOSRootAction(", 1)[1].split(
            "func pairSelectedPhysicalDisk()", 1
        )[0]
        self.assertIn("if linkedCTFPath != nil", mac_action)
        self.assertIn("guard var pinned = linkedFields()", mac_action)
        self.assertIn('pinned["op"] = "ctf_volumes_pinned"', mac_action)
        self.assertIn("self.mountActionGeneration == actionToken", mac_action)
        self.assertIn("self.physicalMountEventGeneration == mountEventToken", mac_action)
        self.assertIn("pendingMountCopy = (disk, scope, openInFinder)", mac_action)
        self.assertNotIn("relative_path", mac_action)
        self.assertIn("secondProbe.filter({ $0.id == verifiedID }).count == 1", MAC)
        self.assertIn("freshMatch.currentMount == mounted.currentMount", MAC)
        self.assertIn("NSWorkspace.shared.open(url)", MAC)
        self.assertIn('Button("Open verified disk in Finder")', MAC)
        self.assertIn('Button("Finder") { model.openCurrentOSMountRoot() }', MAC)
        self.assertIn('.keyboardShortcut("b", modifiers: [.command, .shift])', MAC)

    def test_windows_selection_is_driven_by_current_page(self):
        self.assertIn("fn disk_locator_text(", WIN)
        self.assertIn("entry: Option<&Value>", WIN)
        self.assertIn("header.code == LVN_ITEMCHANGED", WIN)
        self.assertIn("app.update_disk_locator();", WIN)
        self.assertIn("self.entries.get(index as usize)", WIN)
        self.assertIn("ES_READONLY as u32", WIN)
        self.assertIn("visible && window_width >= 1120 && maximum >= 220", WIN)
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
        self.assertIn("FindFirstVolumeW(", WIN)
        self.assertIn("FindNextVolumeW(", WIN)
        self.assertIn("FindVolumeClose(", WIN)
        self.assertIn("GetVolumePathNamesForVolumeNameW(", WIN)
        self.assertIn("GetVolumeInformationW(", WIN)
        self.assertIn("windows_volume_guid_identity(", WIN)
        self.assertIn("all_volume_probe_tests", WIN)
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
        self.assertIn('@AppStorage("AtlasCatalog.locatorVisible") private var locatorVisible = true', MAC)
        self.assertIn('Button(locatorVisible ? "Hide locator" : "Show locator")', MAC)
        self.assertIn('if locatorVisible {\n                    DiskLocatorInspector(model: model)', MAC)
        self.assertIn('.keyboardShortcut("i", modifiers: [.command, .shift])', MAC)
        self.assertIn("minWidth: locatorVisible ? 1100 : 880", MAC)
        self.assertNotIn("self.volume = None;", WIN.split("TOGGLE_LOCATOR =>", 1)[1].split("PHYSICAL_CHECK =>", 1)[0])
        self.assertNotIn("model.volumeID =", MAC.split('Button(locatorVisible ? "Hide locator" : "Show locator")', 1)[1].split(".font(.caption)", 1)[0])

    def test_page_sort_is_explicitly_bounded_and_selection_stays_by_entry_id(self):
        self.assertIn("fn sort_page_rows(", WIN)
        self.assertIn("header.code == LVN_COLUMNCLICK", WIN)
        self.assertIn("self.sort_loaded_page();", WIN)
        self.assertIn("current page only", WIN)
        self.assertIn("page_sort_and_layout_tests", WIN)
        self.assertIn("var displayEntries: [Entry]", MAC)
        self.assertIn("Table(model.displayEntries, selection: $model.selection)", MAC)
        self.assertIn('Picker("Sort current page", selection: $model.pageSortColumn)', MAC)
        self.assertIn("PAGE ONLY", MAC)

    def test_locator_view_preference_is_separate_from_ctf_and_pairings(self):
        self.assertIn('config_dir().join("ui-layout.json")', WIN)
        self.assertIn("fn parse_ui_layout_preferences(", WIN)
        self.assertIn("self.save_ui_layout_preferences();", WIN)
        self.assertIn('@AppStorage("AtlasCatalog.locatorVisible")', MAC)

    def test_incomplete_os_probe_fails_closed_on_both_native_platforms(self):
        self.assertIn("fn scannable_windows_drive_type(", WIN)
        self.assertIn("DRIVE_NO_ROOT_DIR => None", WIN)
        self.assertIn("None => return None, // incomplete OS probe: cannot assert offline", WIN)
        self.assertIn("physical_probe_completeness_tests", WIN)
        self.assertIn("GetLastError() == ERROR_NO_MORE_FILES", WIN)
        self.assertIn("if FindVolumeClose(handle) == 0", WIN)
        self.assertIn("options: []", MAC)
        self.assertNotIn("options: [.skipHiddenVolumes]", MAC)
        self.assertIn("message.wParam == b'R' as usize", WIN)
        self.assertIn("PHYSICAL_CHECK, 0", WIN)
        self.assertIn(".volumeIsLocalKey", MAC)
        self.assertIn("if !isLocal { continue }", MAC)
        self.assertIn("!uuid.isEmpty else { return nil }", MAC)
        self.assertNotIn("return urls.compactMap", MAC)
        self.assertIn('Button("Refresh OS mounts")', MAC)
        self.assertIn('.keyboardShortcut("r", modifiers: [.command])', MAC)
        self.assertIn("OS scan incomplete: physical presence not verified.", MAC)

    def test_cross_disk_candidate_focus_is_bounded_metadata_only(self):
        SERVICE = (ROOT / "crates/atlas-service/src/copy_candidates.rs").read_text(encoding="utf-8")
        self.assertIn('candidate_volume_id', SERVICE)
        self.assertIn('disk_groups', SERVICE)
        self.assertIn('source_volume_id', SERVICE)
        self.assertIn('all_total', SERVICE)
        self.assertIn('exact_normalized_name_and_size', SERVICE)
        self.assertIn('"verified": false', SERVICE)
        self.assertIn('ctf_mirror::read_pinned(request', SERVICE)
        self.assertIn('notification == CBN_SELCHANGE as usize && self.copy_source.is_some()', WIN)
        self.assertIn('self.copy_focus_volume_id = if selected == 0', WIN)
        self.assertIn('self.populate_candidate_disk_picker();', WIN)
        self.assertIn('linked["candidate_volume_id"]', WIN)
        self.assertIn('COPIES BY CATALOG DISK (metadata only)', WIN)
        self.assertIn('ShowWindow(self.help, help_visibility);', WIN)
        self.assertIn("MoveWindow(self.copy_disk_filter, 8, 62, copy_filter_width, 240, 1);", WIN)
        self.assertIn("(w - 634).max(100)", WIN)
        self.assertIn("MoveWindow(*control, w - 616, 62, 146, 23, 1);", WIN)
        self.assertIn('struct CandidateDiskGroup: Decodable, Identifiable', MAC)
        self.assertIn('func focusCopyCandidates(on volume: Int64?)', MAC)
        self.assertIn('args["candidate_volume_id"] = copyFocusVolumeID', MAC)
        self.assertIn('ScrollView(.horizontal)', MAC)
        self.assertIn('Choose by exact catalog ID and your sticker/shelf tag; metadata candidates only.', MAC)

    def test_copy_disk_picker_rechecks_real_os_pairing_before_auto_focus(self):
        self.assertIn("fn sole_connected_candidate_disk(", WIN)
        self.assertIn("verified_copy_focus_tests", WIN)
        self.assertIn("candidate_mount_badge", WIN)
        self.assertIn("COPY_CONNECTED => self.focus_sole_connected_copy()", WIN)
        self.assertIn("MoveWindow(*control, w - 304, 62, 142, 23, 1);", WIN)
        self.assertIn("if self.copy_source.is_some()", WIN)
        self.assertIn("ShowWindow(", WIN)
        focus = WIN.split("unsafe fn focus_sole_connected_copy(", 1)[1].split("unsafe fn populate_candidate_disk_picker", 1)[0]
        self.assertIn("self.refresh_physical_presence();", focus)
        self.assertIn("self.physical_probe_ok", focus)
        self.assertIn("self.populate_candidate_disk_picker();", WIN.split("if old_states != self.physical_states", 1)[1].split("unsafe fn selected_physical_catalog_disk", 1)[0])
        self.assertIn("message.wParam == b'G' as usize", WIN)
        candidate_caption = MAC.split("func candidateCopyDiskCaption(_ group: CandidateDiskGroup)", 1)[1].split("func candidateCopyDiskAccessibility", 1)[0]
        self.assertIn("candidateCopyOSCaption(group.volume_id)", candidate_caption)
        self.assertIn("model.focusSoleConnectedCopy()", MAC)
        self.assertIn("pendingConnectedCopyFocus = (source: source, scope: physicalScope)", MAC)
        self.assertIn("pending.scope == scope", MAC)
        self.assertIn("self.physicalCheckGeneration == checkToken", MAC)
        self.assertIn("source == self.copySourceID", MAC)
        self.assertIn('Button("Use verified mounted disk")', MAC)

    def test_user_shelf_tags_are_native_but_never_mount_evidence(self):
        physical = (ROOT / "crates/atlas-service/src/physical_disks.rs").read_text(encoding="utf-8")
        self.assertIn('physical_shelf_labels', SERVICE)
        self.assertIn('physical_shelf_label_set', SERVICE)
        self.assertIn("shelf_tags_are_private_explicit_generation_scoped_and_not_mount_evidence", physical)
        self.assertIn("ON CONFLICT(catalog_key,platform,volume_id)", physical)
        self.assertIn("SHELF_SAVE => self.save_shelf_label()", WIN)
        self.assertIn("OWNER-ENTERED SHELF/STICKER TAG (not USB identity)", WIN)
        self.assertIn("self.refresh_shelf_labels();", WIN)
        self.assertIn("func saveShelfLabel(on volume: Int64, text: String)", MAC)
        self.assertIn("model.saveShelfLabel(on: diskID, text: shelfDraft)", MAC)
        self.assertIn("if let diskID = model.selectedDiskForPairing", MAC)
        self.assertIn("@ViewBuilder private var selectedFileDetails: some View", MAC)
        self.assertIn("@ViewBuilder private var shelfTagEditor: some View", MAC)
        self.assertIn("@ViewBuilder private var liveMountControls: some View", MAC)
        self.assertIn(".onChange(of: model.selectedDiskForPairing) { id in", MAC)
        self.assertIn("self.shelfLabels = [:]; self.selectedPhysicalID = """, MAC)
        self.assertIn("OWNER STICKER / SHELF TAG (manual, not OS identity)", MAC)
        self.assertIn("(shelfLabels[$0.id] ?? \"\").localizedStandardContains(needle)", MAC)

    def test_ux6_retrieval_note_and_private_shelf_filter_are_native(self):
        self.assertIn("const COPY_LOCATOR: usize = 135", WIN)
        self.assertIn("COPY_LOCATOR => {", WIN)
        self.assertIn("SendMessageW(self.locator, WM_COPY, 0, 0)", WIN)
        self.assertIn("GetKeyState(VK_SHIFT as i32) < 0", WIN)
        self.assertIn("filtered_disk_indices(&self.volumes, &self.shelf_labels, &self.disk_filter_query)", WIN)
        self.assertIn('wide("Filter labels, IDs, shelf tags • Ctrl+L")', WIN)
        self.assertIn('private func retrievalNote(for entry: Entry) -> String', MAC)
        self.assertIn('Button("Copy retrieval note")', MAC)
        self.assertIn('.keyboardShortcut("c", modifiers: [.command, .shift])', MAC)
        self.assertIn("model.shelfLabel(entry.volume_id)", MAC)
        self.assertIn("model.physicalCaption(entry.volume_id)", MAC)
        self.assertIn("file contents not compared", MAC)
        self.assertIn("physical_shelf_labels", SERVICE)

    def test_ux7_exact_catalog_disk_reveal_preserves_search_scope(self):
        self.assertIn("const REVEAL_DISK: usize = 136", WIN)
        self.assertIn("fn reveal_disk_filter_for_row(", WIN)
        self.assertIn('needle.strip_prefix("id:")', WIN)
        self.assertIn("selected_file_reveals_only_its_exact_existing_catalog_disk", WIN)
        self.assertIn("REVEAL_DISK => {", WIN)
        self.assertIn("(*app).command(REVEAL_DISK, 0)", WIN)
        win_reveal = WIN.split("REVEAL_DISK => {", 1)[1].split("COPY_CONNECTED =>", 1)[0]
        self.assertIn("self.disk_filter_query = filter.clone()", win_reveal)
        self.assertNotIn("self.volume =", win_reveal)
        self.assertNotIn("self.load()", win_reveal)
        self.assertIn('needle.lowercased().hasPrefix("id:")', MAC)
        self.assertIn('Button("Show disk in register")', MAC)
        self.assertIn('.keyboardShortcut("j", modifiers: [.command])', MAC)
        mac_reveal = MAC.split("func revealSelectedDiskInRegister()", 1)[1].split("var selectedDiskForPairing", 1)[0]
        self.assertIn("diskFilter =", mac_reveal)
        self.assertNotIn("volumeID =", mac_reveal)
        self.assertNotIn("browseDisk(", mac_reveal)
        self.assertNotIn("load()", mac_reveal)

    def test_ux8_retrieval_candidates_show_private_stickers_without_assuming_mount(self):
        self.assertIn("fn candidate_disk_choice_caption(", WIN)
        self.assertIn("candidate_picker_keeps_exact_disk_sticker_and_uncertain_os_state", WIN)
        picker = WIN.split("unsafe fn populate_candidate_disk_picker(", 1)[1].split("unsafe fn update_disk_locator(", 1)[0]
        self.assertIn("candidate_disk_choice_caption(group, &self.shelf_labels, &self.physical_states)", picker)
        refresh = WIN.split("unsafe fn refresh_shelf_labels(", 1)[1].split("unsafe fn save_shelf_label(", 1)[0]
        self.assertIn("self.populate_candidate_disk_picker();", refresh)
        self.assertIn("func candidateCopyDiskCaption(_ group: CandidateDiskGroup)", MAC)
        self.assertIn("func candidateCopyDiskAccessibility(_ group: CandidateDiskGroup)", MAC)
        self.assertIn("Button(model.candidateCopyDiskCaption(group))", MAC)
        self.assertIn(".accessibilityLabel(model.candidateCopyDiskAccessibility(group))", MAC)
        refresh_mac = MAC.split("func refreshShelfLabels()", 1)[1].split("func saveShelfLabel(", 1)[0]
        self.assertIn("shelfLabels = [:]", refresh_mac)
        self.assertIn("self.shelfLabelRequestGeneration == requestToken", refresh_mac)
        self.assertIn("self.physicalScope == scope", refresh_mac)
        self.assertIn("catalogIDs.contains(row.volume_id)", refresh_mac)
        self.assertIn("shelfLabelRequestGeneration += 1", MAC.split("func saveShelfLabel(", 1)[1].split("func physicalState(", 1)[0])
        self.assertIn("Content identity not verified.", MAC)

    def test_ux9_physical_pickup_list_is_local_sorted_and_keyboard_accessible(self):
        self.assertIn("const COPY_PICKUP_PLAN: usize = 137", WIN)
        self.assertIn("fn ordered_candidate_disks<'a>(", WIN)
        self.assertIn("fn candidate_disk_pickup_list(", WIN)
        self.assertIn("pickup_list_prioritizes_confirmed_mounts_and_keeps_conflicts_explicit", WIN)
        self.assertIn("COPY_PICKUP_PLAN => {", WIN)
        self.assertIn("(*app).command(COPY_PICKUP_PLAN, 0)", WIN)
        self.assertIn("SendMessageW(self.locator, WM_COPY, 0, 0)", WIN)
        copy_action = WIN.split("COPY_PICKUP_PLAN => {", 1)[1].split("COPY_LOCATOR => {", 1)[0]
        self.assertIn("self.update_disk_locator();", copy_action)
        self.assertIn("ordered_candidate_disks(&self.copy_disk_groups, &self.physical_states)", WIN)
        self.assertIn("var orderedCopyDiskGroups: [CandidateDiskGroup]", MAC)
        self.assertIn("func copyPickupPlan()", MAC)
        self.assertIn('Button("Copy pickup list") { model.copyPickupPlan() }', MAC)
        self.assertIn('.keyboardShortcut("p", modifiers: [.command, .shift])', MAC)
        self.assertIn("ForEach(model.orderedCopyDiskGroups)", MAC)
        self.assertIn("Matching name+size never verifies file bytes or hashes; do NOT auto-delete.", WIN)
        self.assertIn("Matching name+size never verifies file bytes or hashes; do NOT auto-delete.", MAC)

    def test_ux10_stale_pages_are_hidden_and_only_current_copy_paths_exported(self):
        self.assertIn("const COPY_PAGE_MANIFEST: usize = 138", WIN)
        self.assertIn("fn candidate_page_manifest(", WIN)
        self.assertIn("page_manifest_uses_current_slice_and_scrubs_untrusted_line_breaks", WIN)
        win_load = WIN.split("unsafe fn load(&mut self) {", 1)[1].split("unsafe fn poll(&mut self) {", 1)[0]
        self.assertIn("self.page_ready = false;", win_load)
        self.assertIn("self.entries.clear();", win_load)
        self.assertIn("LVM_DELETEALLITEMS", win_load)
        self.assertIn("self.page_ready = true;", WIN.split('"rows" => {', 1)[1])
        self.assertIn("COPY_PAGE_MANIFEST => {", WIN)
        self.assertIn("if !self.page_ready || self.copy_source.is_none()", WIN)
        self.assertIn("(*app).command(COPY_PAGE_MANIFEST, 0)", WIN)
        self.assertIn("MoveWindow(*control, w - 616, 62, 146, 23, 1)", WIN)
        self.assertIn("self.update_disk_locator();", WIN.split("COPY_PAGE_MANIFEST => {", 1)[1].split("COPY_PICKUP_PLAN => {", 1)[0])
        self.assertIn("@Published var pageReady = false", MAC)
        mac_load = MAC.split("func load() {", 1)[1].split("func applyPage(", 1)[0]
        self.assertIn("entries = []; total = 0; hasMore = false; pageReady = false", mac_load)
        self.assertIn("pageReady = true", MAC.split("func applyPage(", 1)[1])
        self.assertIn("guard pageReady, copySourceID != nil", MAC)
        self.assertIn("func copyCandidatePagePaths()", MAC)
        self.assertIn('Button("Copy page paths") { model.copyCandidatePagePaths() }', MAC)
        self.assertIn('.keyboardShortcut("m", modifiers: [.command, .shift])', MAC)
        self.assertIn(".disabled(!model.pageReady)", MAC)
        self.assertIn("CURRENT PAGE ONLY", WIN)
        self.assertIn("CURRENT PAGE ONLY", MAC)
        self.assertIn("Linked CTF SHA independently rechecked at clipboard action", WIN)
        self.assertIn("Linked CTF SHA independently rechecked at clipboard action", MAC)

    def test_ux12_shared_stickers_are_private_warnings_not_usb_evidence(self):
        self.assertIn("fn shelf_collisions(", PHYSICAL)
        self.assertIn("let collisions = shelf_collisions(&visible);", PHYSICAL)
        self.assertIn('{"items":items,"collisions":collisions}', PHYSICAL)
        self.assertIn("repeated_stickers_are_scoped_warnings_not_mount_evidence", PHYSICAL)
        self.assertIn("fn shelf_collision_warning(", WIN)
        self.assertIn("shelf_collision_view_tests", WIN)
        self.assertIn("self.shelf_tag_conflicts.clear();", WIN)
        self.assertIn("&self.shelf_tag_conflicts,", WIN)
        self.assertIn("struct ShelfCollisionGroup: Decodable", MAC)
        self.assertIn("@Published var shelfTagConflicts:", MAC)
        self.assertIn("func shelfConflictCaption(", MAC)
        self.assertIn(".accessibilityLabel(warning)", MAC)
        self.assertIn("SHARED STICKER", MAC)

    def test_ux13_candidate_register_is_view_only_and_exact_id_on_both_clients(self):
        self.assertIn("const COPY_REGISTER_FILTER: usize = 139", WIN)
        self.assertIn("fn candidate_register_indices(", WIN)
        self.assertIn("candidate_only_register_uses_exact_ids_and_preserves_original_row_mapping", WIN)
        self.assertIn('eq_ignore_ascii_case("copies:")', WIN)
        self.assertIn("COPY_REGISTER_FILTER => {", WIN)
        self.assertIn("(*app).command(COPY_REGISTER_FILTER, 0)", WIN)
        self.assertRegex(WIN, r"candidate_register_indices\(\s*&self\.volumes,\s*&self\.copy_disk_groups,\s*&self\.physical_states,")
        self.assertIn('self.disk_filter_query = "copies:".to_owned();', WIN)
        rows_handler = WIN.split('"rows" => {', 1)[1].split('"maintenance" => {', 1)[0]
        self.assertIn('eq_ignore_ascii_case("copies:")', rows_handler)
        self.assertIn("self.populate_physical_disk_list();", rows_handler)
        new_copy = WIN.split("unsafe fn show_copies_of_selected(", 1)[1].split("unsafe fn ", 1)[0]
        self.assertIn("self.copy_disk_groups.clear();", new_copy)
        self.assertIn("self.populate_physical_disk_list();", new_copy)
        self.assertIn('if self.copy_source.is_none()', WIN)
        self.assertIn('if needle.lowercased() == "copies:"', MAC)
        self.assertIn("guard copySourceID != nil else { return [] }", MAC)
        self.assertIn("func revealCopyCandidateDisksInRegister()", MAC)
        self.assertIn('Button("Candidates")', MAC)
        self.assertIn('.keyboardShortcut("k", modifiers: [.command, .shift])', MAC)
        self.assertIn('if copySourceID == nil', MAC)
        self.assertIn("Search, selected file and OS pairing remain unchanged.", MAC)

    def test_ux14_candidate_register_is_prioritized_retrieval_not_content_evidence(self):
        self.assertIn("fn candidate_register_row_caption(", WIN)
        self.assertIn("candidate_register_prioritizes_os_verified_then_counts_without_fuzzy_ids", WIN)
        self.assertIn("candidate_register_caption_tests", WIN)
        self.assertIn("candidate_disk_priority(candidates[&disk], states)", WIN)
        self.assertIn("&self.physical_states,", WIN)
        self.assertIn("if candidate_mode {", WIN)
        self.assertIn("name+size metadata candidates", WIN)
        self.assertIn("orderedCopyDiskGroups.compactMap { group -> Volume? in", MAC)
        self.assertIn("seen.insert(group.volume_id).inserted", MAC)
        self.assertIn("func candidateRegisterCount(", MAC)
        self.assertIn("func candidateRegisterAccessibility(", MAC)
        self.assertIn("model.isCandidateRegisterMode", MAC)
        self.assertIn(".accessibilityElement(children: .combine)", MAC)
        self.assertIn("content unverified", MAC)

    def test_ux15_dense_kind_column_and_safe_keyboard_results_focus(self):
        self.assertIn('const SORT_LABELS: [&str; 7]', WIN)
        self.assertIn('("Kind", 70)', WIN)
        self.assertIn('string(e, "kind")', WIN)
        self.assertIn('fn kind_page_sort_distinguishes_catalog_entry_kinds_without_content_probing()', WIN)
        self.assertIn('fn results_focus_rejects_pending_or_empty_page_and_ignores_stale_row()', WIN)
        self.assertIn('fn focusable_result_row(', WIN)
        self.assertIn('FOCUS_RESULTS => self.focus_current_page_results()', WIN)
        self.assertIn('message.wParam == b\'E\' as usize', WIN)
        self.assertIn('LVM_ENSUREVISIBLE', WIN)
        self.assertIn('case "kind":', MAC)
        self.assertIn('TableColumn("Kind")', MAC)
        self.assertIn('Text("Kind").tag("kind")', MAC)
        self.assertIn('func focusCurrentPageResults() -> Bool', MAC)
        self.assertIn('guard pageReady, let first = displayEntries.first', MAC)
        self.assertIn('.focused($resultTableFocused)', MAC)
        self.assertIn('.keyboardShortcut("e", modifiers: [.command])', MAC)
        self.assertIn('if !ready { resultTableFocused = false }', MAC)

    def test_ux16_video_preset_is_core_query_in_two_native_clients(self):
        parser = (ROOT / "crates/atlas-core/src/query.rs").read_text(encoding="utf-8")
        catalog = (ROOT / "crates/atlas-core/tests/catalog.rs").read_text(encoding="utf-8")
        self.assertIn('"type" if v == "video" => Ok(VIDEO_EXTENSION_SQL.into())', parser)
        self.assertIn('"type" => Err(Error::new(', parser)
        self.assertIn('video_extension_search_across_disconnected_disks_is_metadata_only', catalog)
        self.assertIn('fn toggle_video_query(', WIN)
        self.assertIn('VIDEO_FILTER => {', WIN)
        self.assertIn('self.command(FIND, 0);', WIN)
        self.assertIn("message.wParam == b'V' as usize", WIN)
        self.assertIn('func toggleVideoFormatSearch()', MAC)
        self.assertIn('query = next', MAC)
        self.assertIn('.keyboardShortcut("v", modifiers: [.command, .shift])', MAC)
        self.assertIn('model.videoFilterActive ? "All formats" : "Video files"', MAC)

    def test_ux17_literal_same_filename_across_disks_any_size_with_back(self):
        query = (ROOT / "crates/atlas-core/src/query.rs").read_text(encoding="utf-8")
        tests = (ROOT / "crates/atlas-core/tests/catalog.rs").read_text(encoding="utf-8")
        self.assertIn('"name_exact" => {', query)
        self.assertIn('Ok("name_norm = ?".into())', query)
        self.assertIn('literal_filename_lookup_spans_disks_and_ignores_size_without_hash_claims', tests)
        self.assertIn('fn exact_name_query(name: &str) -> Option<String>', WIN)
        self.assertIn('fn search_same_filename(', WIN)
        self.assertIn('if !self.page_ready || self.copy_source.is_some()', WIN)
        self.assertIn('SAME_FILENAME => self.search_same_filename()', WIN)
        self.assertIn("message.wParam == b'N' as usize", WIN)
        self.assertIn('self.copy_return = Some(CopyReturn {', WIN)
        self.assertIn('func searchSameFilenameAcrossDisks()', MAC)
        self.assertIn('guard pageReady, copySourceID == nil', MAC)
        self.assertIn('func exactFilenameQuery(_ name: String) -> String?', MAC)
        self.assertIn('Button("Find same filename (any size)")', MAC)
        self.assertIn('.keyboardShortcut("n", modifiers: [.command, .shift])', MAC)
        self.assertIn('copyReturn = CopyReturnContext(volumeID: volumeID, stack: stack,', MAC)

    def test_ux18_renamed_file_size_candidates_are_exact_global_and_read_only(self):
        parser = (ROOT / "crates/atlas-core/src/query.rs").read_text(encoding="utf-8")
        catalog = (ROOT / "crates/atlas-core/tests/catalog.rs").read_text(encoding="utf-8")
        self.assertIn('"kind" if v == "file" =>', parser)
        self.assertIn('same_recorded_size_search_finds_renamed_files_across_offline_disks_only', catalog)
        self.assertIn('fn exact_recorded_size_query(size: Option<u64>) -> Option<String>', WIN)
        self.assertIn('positive_exact_bytes_only_and_overflow_refused', WIN)
        self.assertIn('if !self.page_ready || self.copy_source.is_some()', WIN)
        self.assertIn('SAME_RECORDED_SIZE => self.search_same_recorded_size()', WIN)
        self.assertIn("message.wParam == b'S' as usize", WIN)
        self.assertIn('GLOBAL exact byte-size file candidates', WIN)
        self.assertIn('self.copy_return = Some(CopyReturn {', WIN)
        self.assertIn('func exactRecordedSizeQuery(_ size: Int64?) -> String?', MAC)
        self.assertIn('func searchSameRecordedSizeAcrossDisks()', MAC)
        self.assertIn('guard pageReady, copySourceID == nil', MAC)
        self.assertIn('Button("Find same byte size (any filename)")', MAC)
        self.assertIn('.keyboardShortcut("s", modifiers: [.command, .shift])', MAC)
        self.assertIn('EXACT RECORDED SIZE • ALL DISKS • FILE CONTENT UNVERIFIED', MAC)
        self.assertIn('copyReturn = CopyReturnContext(volumeID: volumeID, stack: stack,', MAC)
        self.assertIn('load() // source SHA is freshly checked', MAC)

    def test_ux19_physical_register_audit_both_native_clients(self):
        self.assertIn("fn audited_physical_state<'a>(", WIN)
        self.assertIn("fn audited_register_filter_indices(", WIN)
        self.assertIn("fn register_audit_counts(", WIN)
        self.assertIn("physical_audit_filters_keep_exact_ids_and_fail_closed_on_scan_failure", WIN)
        self.assertIn('self.physical_probe_ok,', WIN)
        self.assertIn('PHYSICAL_CONNECTED_FILTER | PHYSICAL_UNVERIFIED_FILTER =>', WIN)
        self.assertIn('self.refresh_physical_presence();', WIN)
        self.assertIn('message.wParam == b\'O\' as usize', WIN)
        self.assertIn('message.wParam == b\'U\' as usize', WIN)
        self.assertIn('register_audit_counts(', WIN)
        self.assertIn('var registerAuditCounts:', MAC)
        self.assertIn('var registerAuditCaption: String', MAC)
        self.assertIn('var registerAuditAccessibility: String', MAC)
        self.assertIn('func showPhysicalStateInRegister(_ state: String)', MAC)
        self.assertIn('refreshPhysicalPresence() // asynchronous:', MAC)
        self.assertIn('guard physicalProbeOK,', MAC)
        self.assertIn("now - verifiedAt <= physicalStateTTL", MAC)
        self.assertIn('return "unverified"', MAC)
        self.assertIn('if lower.hasPrefix("state:")', MAC)
        self.assertIn('if lower.hasPrefix("tag:")', MAC)
        self.assertIn('.keyboardShortcut("o", modifiers: [.command, .shift])', MAC)
        self.assertIn('.keyboardShortcut("u", modifiers: [.command, .shift])', MAC)
        self.assertIn('model.registerAuditAccessibility', MAC)
        self.assertIn('shelfTagConflicts[disk.id]', MAC)
        # Trust boundary remains in the same independent service, not a path heuristic.
        self.assertIn('if !probe_ok {', PHYSICAL)
        self.assertIn('return "unverified";', PHYSICAL)
        self.assertIn('match seen.get(&binding.physical_id)', PHYSICAL)

    def test_ux20_stop_waiting_discards_stale_async_pages_on_both_native_clients(self):
        # No service-level cancellation is claimed, and no historical page can become current.
        self.assertIn("fn page_response_current(", WIN)
        self.assertIn("stopped_or_superseded_page_can_never_reactivate_old_rows", WIN)
        self.assertIn("self.generation = self.generation.wrapping_add(1);", WIN)
        self.assertIn("!page_response_current(reply.generation, self.generation, self.page_awaiting)", WIN)
        self.assertIn('if reply.tag == "rows" {', WIN)
        self.assertIn("self.page_awaiting = false;", WIN)
        self.assertIn("self.page_retry_available = reply.result[\"ok\"] != true;", WIN)
        self.assertIn("STOP_PAGE_WAIT => self.stop_waiting_for_page()", WIN)
        self.assertIn("RETRY_PAGE => {", WIN)
        self.assertIn("LVM_DELETEALLITEMS", WIN.split("unsafe fn stop_waiting_for_page(", 1)[1].split("unsafe fn poll(", 1)[0])
        self.assertIn("self.copy_disk_groups.clear();", WIN.split("unsafe fn stop_waiting_for_page(", 1)[1].split("unsafe fn poll(", 1)[0])
        self.assertIn("message.wParam == b'X' as usize", WIN)
        self.assertIn("message.wParam == b'Y' as usize", WIN)
        self.assertIn("func stopWaitingForPage()", MAC)
        self.assertIn("func retryCurrentPage()", MAC)
        self.assertIn("func acceptPage(_ page: EntryPage?, token: Int)", MAC)
        self.assertIn("guard generation == token else { return }", MAC.split("func acceptPage(", 1)[1].split("func applyPage(", 1)[0])
        self.assertIn("entries = []; copyDiskGroups = []", MAC.split("func stopWaitingForPage()", 1)[1].split("func retryCurrentPage()", 1)[0])
        self.assertIn("self?.acceptPage(reply?.result, token: token)", MAC)
        self.assertIn("self?.acceptPage(answer?.result, token: token)", MAC)
        self.assertIn("self?.acceptPage(page, token: token)", MAC)
        self.assertIn('Button("Stop waiting") { model.stopWaitingForPage() }', MAC)
        self.assertIn('Button("Retry page") { model.retryCurrentPage() }', MAC)
        self.assertIn('.keyboardShortcut("x", modifiers: [.command, .shift])', MAC)
        self.assertIn('.keyboardShortcut("y", modifiers: [.command, .shift])', MAC)
        self.assertIn("Background read may finish", MAC)
        self.assertIn("source read may continue", WIN)

    def test_ux21_other_media_is_service_filtered_before_page_and_sha_pinned(self):
        self.assertIn('linked["other_disks_only"] = json!(true);', WIN)
        self.assertIn('"other_disks_only":true,', WIN)
        self.assertIn('args["other_disks_only"] = true', MAC)
        self.assertIn('COPY CANDIDATES • OTHER CATALOG DISKS ONLY', MAC)
        self.assertIn('OTHER catalog disks only', WIN)
        self.assertIn('scope_clause.push_str(&format!(" AND volume_id<>?', COPIES)
        self.assertIn('"other_total": other_total', COPIES)
        self.assertIn('"other_disks_only": other_disks_only', COPIES)
        self.assertIn('ctf_mirror::read_pinned(request,', COPIES)
        pinned = (ROOT / "crates/atlas-service/tests/ctf_pinned_duplicates_contract.rs").read_text(encoding="utf-8")
        self.assertIn('let stale_other = call(json!({', pinned)
        self.assertIn('assert_eq!(stale_other["error"]["code"], "source_changed");', pinned)

    def test_ux22_live_os_root_requires_service_pairing_and_two_native_probes(self):
        self.assertIn('"verified_physical_id":verified_physical_id', PHYSICAL)
        self.assertIn('let state = classify(id, &known, &seen, probe_ok);', PHYSICAL)
        self.assertIn('if state == "connected"', PHYSICAL)
        self.assertIn('fn verified_mount_root(', WIN)
        self.assertIn('only_explicit_unique_os_verified_identity_exposes_current_mount_root', WIN)
        self.assertIn('self.refresh_physical_presence();', WIN)
        self.assertIn('let from_fresh_probe = windows_mounted_volumes()', WIN)
        self.assertIn('self.linked_request("ctf_volumes_pinned")', WIN)
        self.assertIn('COPY_VERIFIED_OS_ROOT =>', WIN)
        self.assertIn("message.wParam == b'H' as usize", WIN)
        self.assertIn('let verified_physical_id: String?', MAC)
        self.assertIn('func copyCurrentOSMountRoot()', MAC)
        self.assertIn('pinned["op"] = "ctf_volumes_pinned"', MAC)
        self.assertIn('let secondProbe = self.scanPhysicalVolumes()', MAC)
        self.assertIn('self.physicalState(pending.disk) == "connected"', MAC)
        self.assertIn('Button("Copy current OS root")', MAC)
        self.assertIn('Button("OS root") { model.copyCurrentOSMountRoot() }', MAC)
        self.assertIn('if self.linked_mode() {', WIN)
        self.assertIn('.keyboardShortcut("h", modifiers: [.command, .shift])', MAC)
        self.assertNotIn('file://' + ' + entry.relative_path', WIN + MAC)

    def test_no_unverified_physical_drive_claims_in_locator(self):
        self.assertIn("Physical connection: NOT VERIFIED", WIN)
        self.assertIn("Physical connection not verified", MAC)
        self.assertIn("not content-verified", WIN)
        self.assertIn("not verified by content", MAC)
        self.assertIn("Linked CTF — read-only", MAC)
        self.assertIn("source read-only; SHA generation pinned", WIN)

    def test_ux23_sleep_resume_monotonic_probe_expiry_and_clone_safe_pairing(self):
        self.assertIn("fn physical_probe_is_fresh(", WIN)
        self.assertIn("stale_os_probes_never_authorize_a_connected_badge", WIN)
        self.assertIn("fn unique_matching_live_mount(", WIN)
        self.assertIn("second_pairing_probe_rejects_a_new_same_uuid_clone", WIN)
        self.assertIn("!unique_matching_live_mount(&candidate, &self.mounted_volumes)", WIN)
        self.assertNotIn("v.id == candidate.id && v.description == candidate.description", WIN)
        self.assertIn("physical_verified_at: Option<Instant>", WIN)
        self.assertIn("self.expire_physical_presence_if_stale();", WIN)
        self.assertIn("WM_ACTIVATEAPP =>", WIN)
        self.assertIn("app.invalidate_physical_presence();", WIN)
        self.assertIn('reply["data"]["probe_ok"] == true', WIN)
        self.assertIn("let scannedAt = ProcessInfo.processInfo.systemUptime", MAC)
        self.assertIn("private var physicalVerifiedUptime: TimeInterval?", MAC)
        self.assertIn("now - verifiedAt <= physicalStateTTL", MAC)
        self.assertIn("self.physicalVerifiedUptime = scannedAt", MAC)
        self.assertIn("self.physicalVerifiedUptime = nil", MAC)
        self.assertIn("NSApplication.didBecomeActiveNotification", MAC)
        self.assertIn("NSWorkspace.didWakeNotification", MAC)
        self.assertIn('self.physicalState(pending.disk) == "connected"', MAC)
    def test_ux24_disk_only_pickup_card_exact_id_stale_os_state_and_native_shortcut(self):
        self.assertIn("fn disk_pickup_card(", WIN)
        self.assertIn("exact_disk_id_and_cyrillic_sticker_survive_duplicate_labels", WIN)
        self.assertIn("stale_or_failed_os_probe_cannot_claim_connected_on_pickup_card", WIN)
        self.assertIn("COPY_DISK_CARD => {", WIN)
        self.assertIn('self.linked_request("ctf_volumes_pinned")', WIN)
        self.assertIn('self.call("disk_card_source", pinned)', WIN)
        self.assertIn('if reply.tag == "disk_card_source"', WIN)
        self.assertIn('generation == reply.generation', WIN)
        self.assertIn('self.selected_physical_catalog_disk() == Some(disk)', WIN)
        self.assertIn("physical_probe_is_fresh(self.physical_verified_at, Instant::now())", WIN)
        self.assertIn('message.wParam == b\'L\' as usize', WIN)
        self.assertIn('Copy selected PHYSICAL disk pickup card', WIN)
        self.assertIn('format!("Catalog disk ID: #{disk_id}")', WIN)
        self.assertIn('func copySelectedDiskPickupCard()', MAC)
        self.assertIn('pinned["op"] = "ctf_volumes_pinned"', MAC)
        self.assertIn('request(pinned, as: LinkedCTFResult<[Volume]>.self, generationToken: token)', MAC)
        self.assertIn('self.selectedDiskForPairing == diskID', MAC)
        self.assertIn('private func writeDiskPickupCard(_ disk: Volume)', MAC)
        self.assertIn('physicalCaption(diskID)', MAC)
        self.assertIn('Button("Copy disk pickup card")', MAC)
        self.assertIn('Button("Disk card")', MAC)
        self.assertIn('.keyboardShortcut("l", modifiers: [.command, .shift])', MAC)
        self.assertIn('"#\\(disk.id) • \\(disk.label)"', MAC)
        self.assertIn('never auto-delete metadata matches', WIN)
        self.assertIn('never auto-delete metadata matches', MAC)

    def test_ux25_candidate_clipboard_repins_linked_source_and_rejects_late_reply(self):
        self.assertIn("struct CandidateClipboardGuard {", WIN)
        self.assertIn("fn candidate_clipboard_reply_current(", WIN)
        self.assertIn("stale_ctf_verification_never_authorizes_a_new_clipboard_page", WIN)
        self.assertIn('self.call("candidate_clipboard_source", pinned);', WIN)
        self.assertIn('self.linked_request("ctf_volumes_pinned")', WIN)
        self.assertIn('reply.result["data"]["generation"].as_str()', WIN)
        self.assertIn("self.pending_candidate_clipboard.take()", WIN)
        self.assertIn("self.write_candidate_clipboard(guard.command);", WIN)
        self.assertIn("source_id == Some(guard.source_id)", WIN)
        self.assertIn("offset == guard.offset", WIN)
        self.assertIn("focus_disk == guard.focus_disk", WIN)
        for action in ("COPY_PAGE_MANIFEST", "COPY_PICKUP_PLAN"):
            self.assertIn("self.verify_candidate_clipboard(" + action + ");", WIN)
        self.assertIn("private func verifyCandidateClipboardThen(", MAC)
        self.assertIn('pinned["op"] = "ctf_volumes_pinned"', MAC)
        self.assertIn("self.candidateClipboardRequestSerial == serial", MAC)
        self.assertIn("self.physicalScope == scope", MAC)
        self.assertIn("self.copySourceID == source", MAC)
        self.assertIn("self.copyFocusVolumeID == focus", MAC)
        self.assertIn("self.offset == page", MAC)
        self.assertIn("reply?.generation == self.linkedGeneration", MAC)
        self.assertIn("verifyCandidateClipboardThen(.pickupList)", MAC)
        self.assertIn("verifyCandidateClipboardThen(.pagePaths)", MAC)
        self.assertIn("source catalog disk excluded", WIN)
        self.assertIn("source catalog disk excluded", MAC)
        self.assertNotIn("selected file included.", WIN)
        self.assertNotIn("selected file included.", MAC)

    def test_ux26_hotplug_mount_events_immediately_revoke_stale_os_identity(self):
        self.assertIn("fn volume_change_response(event: usize)", WIN)
        self.assertIn("physical_usb_mount_changes_fail_closed_before_recheck", WIN)
        change = WIN.split("WM_DEVICECHANGE => {", 1)[1].split("WM_ACTIVATEAPP => {", 1)[0]
        self.assertIn("app.invalidate_physical_presence();", change)
        self.assertIn("app.refresh_physical_presence();", change)
        self.assertIn("VolumeChangeResponse::Invalidate => app.invalidate_physical_presence()", change)
        self.assertNotIn('GetLogicalDrives', change)
        self.assertIn("func invalidatePhysicalPresenceAfterMountEvent()", MAC)
        reset = MAC.split("func invalidatePhysicalPresenceAfterMountEvent()", 1)[1].split(
            "func recheckPhysicalPresenceAfterMountEvent()", 1
        )[0]
        for guard in (
            "physicalCheckGeneration += 1",
            "physicalMountEventGeneration += 1",
            "physicalVerifiedUptime = nil",
            "physicalStates = [:]",
            "verifiedPhysicalIDs = [:]",
            "physicalProbeOK = false",
            "mountedPhysicalVolumes = []",
            "pendingMountCopy = nil",
            "pendingConnectedCopyFocus = nil",
        ):
            self.assertIn(guard, reset)
        for event in ("willUnmountNotification", "didUnmountNotification",
                      "didMountNotification", "didRenameVolumeNotification"):
            self.assertIn("NSWorkspace." + event, MAC)
        self.assertIn("this observer never vetoes safe eject.", WIN)
        self.assertIn("model.invalidatePhysicalPresenceAfterMountEvent()", MAC)
        self.assertIn("model.recheckPhysicalPresenceAfterMountEvent()", MAC)
        self.assertIn('self.physicalCheckGeneration == checkToken', MAC)
        self.assertIn("let mountEventToken = physicalMountEventGeneration", MAC)
        self.assertIn("self.physicalMountEventGeneration == mountEventToken", MAC)

if __name__ == "__main__":
    unittest.main()
