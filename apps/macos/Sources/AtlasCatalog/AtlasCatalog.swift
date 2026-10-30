import SwiftUI
import AppKit
import CAtlas

struct Volume: Decodable, Identifiable {
    let id: Int64
    let label: String
    let root_path: String?
    let entry_count: Int64?
    let total_bytes: Int64?
    let status: String?
}
struct Entry: Decodable, Identifiable {
    let id: Int64
    let volume_id: Int64
    let parent_id: Int64?
    let name: String
    let relative_path: String
    let kind: String
    let size: Int64?
    let modified_unix: Int64?
    let modified_local: String?
    let description: String?
    let duplicate_count: Int64?
    var isFolder: Bool { kind == "directory" || kind == "archive" }
    var sizeText: String { size.map { ByteCountFormatter.string(fromByteCount: $0, countStyle: .file) } ?? "" }
    var dateText: String { modified_local ?? modified_unix.map { Date(timeIntervalSince1970: Double($0)).formatted(date: .numeric, time: .shortened) } ?? "" }
}
struct EntryPage: Decodable { let items: [Entry]; let total: Int?; let offset: Int?; let limit: Int?; let has_more: Bool? }
struct CoreError: Decodable { let code: String?; let message: String }
struct Envelope<T: Decodable>: Decodable { let ok: Bool; let data: T?; let error: CoreError? }
struct EmptyReply: Decodable {}
struct Category: Decodable, Identifiable { let id: Int64; let parent_id: Int64?; let name: String }
struct CreatedCategory: Decodable { let id: Int64 }
struct LinkedCTFSync: Decodable { let generation: String; let entries: Int64; let volumes: Int64; let rebuilt: Bool }
struct LinkedExportReceipt: Decodable { let result: LinkedExportSummary }
struct LinkedExportSummary: Decodable { let rows: Int64 }
struct LinkedCTFResult<T: Decodable>: Decodable { let generation: String; let result: T }
struct AnnotationCandidate: Decodable, Identifiable {
    let entry_id: Int64
    let volume_id: Int64
    let parent_id: Int64?
    let modified_unix: Int64?
    var id: Int64 { entry_id }
}
struct AnnotationConfirmedSelection: Decodable {
    let new_entry_id: Int64
}
struct AnnotationReviewRow: Decodable, Identifiable {
    let old_entry_id: Int64
    let revision: Int64
    let candidate_state: String
    let candidate_count_lower_bound: Int
    let candidates_truncated: Bool
    let candidates: [AnnotationCandidate]
    let confirmed_selection: AnnotationConfirmedSelection?
    let confirmation_revision_conflict: Bool
    let confirmation_payload_conflict: Bool
    // Audit status alone is NOT proof of a verified active projection.
    let activation_state: String?
    let activation_target_entry_id: Int64?
    var id: Int64 { old_entry_id }
    var canActivate: Bool {
        guard activation_state == nil, revision > 0,
              !confirmation_revision_conflict, !confirmation_payload_conflict,
              let target = confirmed_selection?.new_entry_id else { return false }
        return candidates.contains { $0.entry_id == target }
    }
    var canRevoke: Bool {
        activation_state == "active" && activation_target_entry_id != nil
    }
    var canSelect: Bool {
        revision > 0 && confirmed_selection == nil && !candidates.isEmpty
    }
    var conflictText: String {
        if activation_state == "active" {
            return confirmation_revision_conflict || confirmation_payload_conflict
                ? "Active audit is stale — revoke to clear"
                : "Active audit recorded — independently verified on read"
        }
        if activation_state == "revoked" {
            return "Revoked — audit retained; cannot replay"
        }
        if confirmation_revision_conflict || confirmation_payload_conflict {
            return "Stale confirmation — resolve separately"
        }
        if let confirmed_selection {
            return "Confirmed #\(confirmed_selection.new_entry_id); inactive"
        }
        return "Unconfirmed — explicit approval required"
    }
}
struct AnnotationReview: Decodable {
    let state: String
    let document_id: String?
    let total: Int64
    let items: [AnnotationReviewRow]
}
struct AnnotationConfirmReceipt: Decodable {
    let state: String
    let old_entry_id: Int64
    let new_entry_id: Int64
}

// Each state requires a separate native consent. Mapping never activates.
enum AnnotationConsent: Equatable {
    case mapping
    case activation(oldID: Int64, newID: Int64)
    case revocation(oldID: Int64, newID: Int64)
}

enum Bridge {
    static func invoke(_ input: Data) -> Data {
        let request = String(decoding: input, as: UTF8.self)
        return request.withCString { ptr in
            guard let output = atlas_call(ptr) else { return Data("{\"ok\":false,\"error\":{\"message\":\"Native core returned no response\"}}".utf8) }
            defer { atlas_string_free(output) }
            return Data(String(cString: output).utf8)
        }
    }
}

@MainActor final class Catalog: ObservableObject {
    @Published var database: String
    @Published var volumes: [Volume] = []
    @Published var volumeID: Int64?
    @Published var entries: [Entry] = []
    @Published var selection: Int64?
    @Published var query = ""
    var appliedQuery = ""
    @Published var status = "Opening catalog…"
    @Published var error: String?
    @Published var pending = 0
    @Published var job: String?
    @Published var duplicateMode = "name_size"
    var appliedDuplicateMode = "name_size"
    @Published var showingDuplicates = false
    @Published var categories: [Category] = []
    @Published var categoryID: Int64?
    @Published var categoryName = ""
    @Published var exportFormat = "csv"
    @Published var linkedCTFPath: String?
    @Published var annotationReview: AnnotationReview?
    @Published var showingAnnotationReview = false
    @Published var annotationReviewOffset = 0
    @Published var annotationReviewLoading = false
    @Published var annotationSelectedOldID: Int64?
    @Published var annotationSelectedCandidateID: Int64?
    @Published var annotationConsent: AnnotationConsent?
    @Published var annotationConfirmationBusy = false
    private var annotationReviewStore: String?
    private var annotationReviewSession = 0
    let annotationPageSize = 20
    var annotationReviewHasPrevious: Bool {
        annotationReview?.state == "generation_conflict" && annotationReviewOffset > 0
    }
    var annotationReviewHasNext: Bool {
        guard let review = annotationReview, review.state == "generation_conflict",
              !review.items.isEmpty else { return false }
        return Int64(annotationReviewOffset + review.items.count) < review.total
    }
    var linkedGeneration: String?
    var linkedMode: Bool { linkedCTFPath != nil && linkedGeneration != nil }
    var editableAllowed: Bool { mutationAllowed && !linkedMode }
    var linkedCacheRoot: String {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("AtlasCatalog/CTF-Mirrors", isDirectory: true).path
    }
    func linkedFields() -> [String: Any]? {
        guard let linkedCTFPath, let linkedGeneration else { return nil }
        return ["path": linkedCTFPath, "cache_root": linkedCacheRoot, "generation": linkedGeneration]
    }
    @Published var offset = 0
    @Published var total = 0
    @Published var hasMore = false
    var stack: [(Int64?, String)] = [(nil, "/")]
    var generation = 0
    let limit = 250
    var currentPath: String { stack.last?.1 ?? "/" }
    var mutationAllowed: Bool { job == nil && pending == 0 }
    var canCancel: Bool { job?.hasPrefix("scan-") == true }
    var selected: Entry? { entries.first { $0.id == selection } }

    init() {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("AtlasCatalog", isDirectory: true)
        try? FileManager.default.createDirectory(at: support, withIntermediateDirectories: true)
        let arguments = CommandLine.arguments.dropFirst().filter { !$0.hasPrefix("-") }
        database = arguments.first ?? UserDefaults.standard.string(forKey: "AtlasCatalog.database") ?? support.appendingPathComponent("Catalog.atlas.sqlite").path
        if atlas_abi_version() != 1 { error = "Unsupported Atlas core ABI. Rebuild the application and native library together."; return }
        initialize()
    }
    func request<T: Decodable>(_ operation: [String: Any], as: T.Type, generationToken: Int? = nil, completion: @escaping (T?) -> Void) {
        let requestDatabase = database
        var args = operation; args["db"] = requestDatabase
        guard let input = try? JSONSerialization.data(withJSONObject: args) else { error = "Could not encode the core request."; return }
        pending += 1
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let output = Bridge.invoke(input)
            DispatchQueue.main.async {
                guard let self else { return }
                self.pending = max(0, self.pending - 1)
                guard self.database == requestDatabase else { return }
                guard generationToken == nil || generationToken == self.generation else { return }
                do {
                    let reply = try JSONDecoder().decode(Envelope<T>.self, from: output)
                    if reply.ok { completion(reply.data) }
                    else { self.error = reply.error?.message ?? "Catalog operation failed."; self.status = self.error ?? "Error"; completion(nil) }
                } catch { self.error = "Invalid response from core: \(error.localizedDescription)"; completion(nil) }
            }
        }
    }
    func initialize() {
        linkedCTFPath = nil; linkedGeneration = nil
        resetAnnotationReview()
        generation += 1; volumeID = nil; stack = [(nil, "/")]; entries = []; query = ""; appliedQuery = ""; showingDuplicates = false; offset = 0
        volumes = []; categories = []; categoryID = nil; selection = nil; total = 0; hasMore = false
        status = "Opening catalog…"
        request(["op": "init"], as: EmptyReply.self) { [weak self] result in
            guard let self, result != nil else { return }
            UserDefaults.standard.set(self.database, forKey: "AtlasCatalog.database")
            self.refreshVolumes()
        }
    }
    func volumeLabel(_ id: Int64) -> String {
        if let label = volumes.first(where: { $0.id == id })?.label, !label.isEmpty { return label }
        return "Disk #\(id)"
    }
    func refreshVolumes() {
        refreshCategories()
        if let fields = linkedFields() {
            var requestData: [String: Any] = ["op": "ctf_volumes_pinned"]
            requestData.merge(fields) { _, newValue in newValue }
            request(requestData, as: LinkedCTFResult<[Volume]>.self) { [weak self] response in
                guard let self, let response else { return }
                self.volumes = response.result
                self.status = "\(response.result.count) linked CTF disks • read-only; search or select a disk"
            }
            return
        }
        request(["op": "volumes"], as: [Volume].self) { [weak self] values in
            guard let self, let values else { return }
            self.volumes = values
            self.status = "\(values.count) disks • choose a disk or search all disks"
        }
    }
    func refreshCategories() {
        if linkedMode { categories = []; categoryID = nil; return }
        request(["op": "categories"], as: [Category].self) { [weak self] values in if let values { self?.categories = values } }
    }
    func createCategory() {
        guard !linkedMode else { status = "Linked CTF is read-only; category edits are not supported."; return }
        let name = categoryName.trimmingCharacters(in: .whitespaces)
        guard !name.isEmpty else { return }
        request(["op": "create_category", "name": name], as: CreatedCategory.self) { [weak self] result in
            guard let self, let result else { return }; self.categoryName = ""; self.categoryID = result.id; self.refreshCategories()
        }
    }
    func filterCategory() {
        guard let category = categories.first(where: { $0.id == categoryID }) else { return }
        query = "category:\"" + category.name.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "\"", with: "\\\"") + "\""; search()
    }
    func assignCategory() {
        guard !linkedMode else { status = "Linked CTF does not accept category writes."; return }
        guard let selection, let categoryID else { return }
        request(["op": "assign_category", "entry_id": selection, "category_id": categoryID], as: EmptyReply.self) { [weak self] result in if result != nil { self?.status = "Category assigned to selected entry." } }
    }
    func selectVolume() { stack = [(nil, "/")]; offset = 0; showingDuplicates = false; load() }
    func load() {
        generation += 1; let token = generation; selection = nil
        var args: [String: Any] = ["offset": offset, "limit": limit]
        if let volumeID { args["volume_id"] = volumeID }
        if let fields = linkedFields() {
            if showingDuplicates {
                args["op"] = "ctf_duplicates_pinned"
                args["duplicate_mode"] = appliedDuplicateMode
                args["query"] = appliedQuery
            } else if !appliedQuery.isEmpty {
                args["op"] = "ctf_search_pinned"; args["query"] = appliedQuery
            } else if volumeID != nil {
                args["op"] = "ctf_children_pinned"
                args["parent_id"] = stack.last.flatMap { $0.0 }.map { $0 as Any } ?? NSNull()
            } else {
                entries = []; total = 0; hasMore = false
                status = "Select a linked CTF disk or search all disks."; return
            }
            args.merge(fields) { _, newValue in newValue }
            status = showingDuplicates
                ? "Finding read-only CTF metadata duplicates (may be slow on huge catalogs)…"
                : "Searching verified linked CTF…"
            request(args, as: LinkedCTFResult<EntryPage>.self, generationToken: token) { [weak self] answer in
                guard let self, let answer else { return }
                self.applyPage(answer.result, token: token)
            }
            return
        }
        if showingDuplicates {
            args["op"] = "duplicates"; args["mode"] = appliedDuplicateMode; args["query"] = appliedQuery
        } else if !appliedQuery.isEmpty {
            args["op"] = "search_fast"; args["query"] = appliedQuery
        } else if volumeID != nil {
            args["op"] = "children"; args["parent_id"] = stack.last.flatMap { $0.0 }.map { $0 as Any } ?? NSNull()
        } else { entries = []; total = 0; hasMore = false; status = "Choose a disk or enter an expression to search all disks."; return }
        status = showingDuplicates ? "Finding duplicate candidates by metadata…" : "Loading…"
        request(args, as: EntryPage.self, generationToken: token) { [weak self] page in
            guard let self, let page else { return }
            self.applyPage(page, token: token)
        }
    }
    func applyPage(_ page: EntryPage, token: Int) {
        guard generation == token else { return }
        entries = page.items; total = page.total ?? (offset + page.items.count)
        hasMore = page.has_more ?? (offset + page.items.count < total)
        status = "\(page.items.isEmpty ? 0 : offset + 1)–\(offset + page.items.count)"
            + (page.total.map { " of \($0) items" } ?? (hasMore ? " items • more available" : " items"))
            + (showingDuplicates ? " • metadata candidates; content not verified" : "")
    }
    func search() { appliedQuery = query.trimmingCharacters(in: .whitespacesAndNewlines); showingDuplicates = false; offset = 0; load() }
    func duplicates() {
        appliedQuery = query.trimmingCharacters(in: .whitespacesAndNewlines); appliedDuplicateMode = duplicateMode; showingDuplicates = true; offset = 0; load() }
    func back() {
        if showingDuplicates || !appliedQuery.isEmpty { showingDuplicates = false; query = ""; appliedQuery = "" }
        else if stack.count > 1 { stack.removeLast() }
        offset = 0; load()
    }
    func openSelected() {
        guard let entry = selected, entry.isFolder else { return }
        if volumeID != entry.volume_id { volumeID = entry.volume_id; stack = [(nil, "/")] }
        stack.append((entry.id, entry.relative_path)); showingDuplicates = false; query = ""; appliedQuery = ""; offset = 0; load()
    }
    func linkCTF() {
        guard mutationAllowed else { return }
        let panel = NSOpenPanel()
        panel.title = "Link external WhereIsIt CTF (read-only)"
        panel.canChooseDirectories = false; panel.allowsMultipleSelection = false
        panel.begin { [weak self] response in
            guard let self, response == .OK, let url = panel.url else { return }
            guard url.pathExtension.lowercased() == "ctf" else {
                self.error = "Choose a WhereIsIt .CTF catalog."; return
            }
            self.synchronizeLinkedCTF(url.path)
        }
    }
    func synchronizeLinkedCTF(_ source: String) {
        guard mutationAllowed else { return }
        resetAnnotationReview()
        job = "linked-sync"
        status = "Verifying external CTF and its derived index…"
        request(["op": "ctf_sync", "path": source, "cache_root": linkedCacheRoot],
                as: LinkedCTFSync.self) { [weak self] result in
            guard let self else { return }
            self.job = nil
            guard let result else { return }
            self.linkedCTFPath = source; self.linkedGeneration = result.generation
            if self.exportFormat == "ctf" { self.exportFormat = "csv" }
            self.volumeID = nil; self.stack = [(nil, "/")]; self.entries = []
            self.query = ""; self.appliedQuery = ""; self.showingDuplicates = false
            self.offset = 0; self.selection = nil; self.total = 0; self.hasMore = false
            self.generation += 1
            self.status = result.rebuilt ? "New CTF mirror verified; source unchanged." : "Verified existing CTF mirror."
            self.refreshVolumes()
        }
    }
    func refreshLinkedCTF() {
        guard let source = linkedCTFPath, mutationAllowed else { return }
        synchronizeLinkedCTF(source)
    }
    func resetAnnotationReview() {
        annotationReviewSession += 1
        annotationReviewStore = nil
        annotationReview = nil
        annotationReviewOffset = 0
        annotationReviewLoading = false
        annotationSelectedOldID = nil
        annotationSelectedCandidateID = nil
        annotationConsent = nil
        annotationConfirmationBusy = false
        showingAnnotationReview = false
    }
    func reviewAnnotations() {
        guard linkedMode, mutationAllowed else { return }
        let linkedAtOpen = linkedGeneration
        let panel = NSOpenPanel()
        panel.title = "Choose your existing .wia.sqlite notes file (preview only)"
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.begin { [weak self] response in
            guard let self, response == .OK, let url = panel.url else { return }
            guard url.lastPathComponent.hasSuffix(".wia.sqlite") else {
                self.error = "Choose an existing .wia.sqlite user annotation file."
                return
            }
            guard self.linkedGeneration == linkedAtOpen, self.mutationAllowed else { return }
            self.resetAnnotationReview()
            self.annotationReviewStore = url.path
            self.showingAnnotationReview = true
            self.loadAnnotationReviewPage(offset: 0)
        }
    }
    func loadAnnotationReviewPage(offset: Int) {
        guard linkedMode, let fields = linkedFields(), let store = annotationReviewStore,
              !annotationReviewLoading, pending == 0, offset >= 0 else { return }
        if let review = annotationReview, review.state == "generation_conflict",
           offset > 0 && Int64(offset) >= review.total { return }
        let session = annotationReviewSession
        let token = generation
        annotationReviewOffset = offset
        annotationReview = nil
        annotationSelectedOldID = nil
        annotationSelectedCandidateID = nil
        annotationConsent = nil
        annotationReviewLoading = true
        var args = fields
        args["op"] = "ctf_annotations_preview_pinned"
        args["annotation_db"] = store
        args["offset"] = offset
        args["limit"] = annotationPageSize
        status = "Verifying source generation and notes page (read-only)…"
        request(args, as: LinkedCTFResult<AnnotationReview>.self,
                generationToken: token) { [weak self] response in
            guard let self, self.annotationReviewSession == session,
                  self.annotationReviewStore == store, self.showingAnnotationReview else { return }
            self.annotationReviewLoading = false
            self.annotationReview = response?.result
            if response != nil {
                self.status = "Read-only notes page checked; nothing was changed."
            }
        }
    }
    var selectedAnnotationRow: AnnotationReviewRow? {
        guard let oldID = annotationSelectedOldID else { return nil }
        return annotationReview?.items.first { $0.old_entry_id == oldID }
    }
    var canConfirmAnnotation: Bool {
        guard showingAnnotationReview, linkedMode, mutationAllowed,
              !annotationReviewLoading, !annotationConfirmationBusy,
              annotationReview?.state == "generation_conflict",
              annotationReview?.document_id != nil,
              let row = selectedAnnotationRow, row.canSelect,
              let selectedID = annotationSelectedCandidateID else { return false }
        return row.candidates.contains { $0.entry_id == selectedID }
    }
    func selectAnnotation(_ row: AnnotationReviewRow) {
        guard row.canSelect, !annotationReviewLoading, !annotationConfirmationBusy else { return }
        annotationSelectedOldID = row.old_entry_id
        // Never preselect a candidate, even when the preview found exactly one.
        annotationSelectedCandidateID = nil
    }
    func confirmAnnotationSelection() {
        guard canConfirmAnnotation, let fields = linkedFields(),
              let store = annotationReviewStore,
              let documentID = annotationReview?.document_id,
              let row = selectedAnnotationRow,
              let targetID = annotationSelectedCandidateID else { return }
        let session = annotationReviewSession
        let token = generation
        let page = annotationReviewOffset
        var args = fields
        args["op"] = "ctf_annotations_confirm_pinned"
        args["annotation_db"] = store
        args["expected_document_id"] = documentID
        args["old_entry_id"] = row.old_entry_id
        args["new_entry_id"] = targetID
        args["expected_revision"] = row.revision
        args["confirmation"] = "user_selected_exact_candidate"
        annotationConfirmationBusy = true
        status = "Verifying current CTF generation and explicit mapping…"
        request(args, as: LinkedCTFResult<AnnotationConfirmReceipt>.self,
                generationToken: token) { [weak self] response in
            guard let self, self.annotationReviewSession == session,
                  self.annotationReviewStore == store, self.showingAnnotationReview else { return }
            self.annotationConfirmationBusy = false
            if let receipt = response?.result, receipt.state == "confirmed_not_activated" {
                self.status = "Mapping #\(receipt.old_entry_id) → #\(receipt.new_entry_id) confirmed; annotation remains inactive."
            } else {
                self.status = "Mapping not confirmed. Review the refreshed conflict and choose again."
            }
            // Successful or failed CAS must always discard stale page selection.
            self.loadAnnotationReviewPage(offset: page)
        }
    }
    func proposeAnnotationOverlay(_ consent: AnnotationConsent) {
        guard showingAnnotationReview, linkedMode, mutationAllowed,
              !annotationReviewLoading, !annotationConfirmationBusy,
              let review = annotationReview, review.state == "generation_conflict",
              review.document_id != nil else { return }
        switch consent {
        case .mapping: return
        case .activation(let oldID, let newID):
            guard review.items.contains(where: {
                $0.old_entry_id == oldID && $0.canActivate &&
                $0.confirmed_selection?.new_entry_id == newID
            }) else { return }
        case .revocation(let oldID, let newID):
            guard review.items.contains(where: {
                $0.old_entry_id == oldID && $0.canRevoke &&
                $0.activation_target_entry_id == newID
            }) else { return }
        }
        annotationConsent = consent
    }
    func applyAnnotationOverlay(_ consent: AnnotationConsent) {
        guard showingAnnotationReview, linkedMode, mutationAllowed,
              !annotationReviewLoading, !annotationConfirmationBusy,
              let fields = linkedFields(), let store = annotationReviewStore,
              let review = annotationReview, review.state == "generation_conflict",
              let documentID = review.document_id else { return }
        var args = fields
        args["annotation_db"] = store
        args["expected_document_id"] = documentID
        let verb: String
        switch consent {
        case .mapping: return
        case .activation(let oldID, let newID):
            guard let row = review.items.first(where: { $0.old_entry_id == oldID }),
                  row.canActivate, row.confirmed_selection?.new_entry_id == newID else { return }
            args["op"] = "ctf_annotations_activate_pinned"
            args["old_entry_id"] = oldID
            args["new_entry_id"] = newID
            args["expected_revision"] = row.revision
            args["activation"] = "user_approved_reversible_projection"
            verb = "Activation"
        case .revocation(let oldID, let newID):
            // Revocation must work even when an activation has become stale.
            // The journal's active pair is the revocation CAS.
            guard let row = review.items.first(where: { $0.old_entry_id == oldID }),
                  row.canRevoke, row.activation_target_entry_id == newID else { return }
            args["op"] = "ctf_annotations_deactivate_pinned"
            args["old_entry_id"] = oldID
            args["new_entry_id"] = newID
            args["rollback"] = "user_confirmed_revoke_projection"
            verb = "Revocation"
        }
        let session = annotationReviewSession
        let token = generation
        let page = annotationReviewOffset
        annotationConsent = nil
        annotationConfirmationBusy = true
        status = "\(verb): rechecking source SHA, document and audit state…"
        request(args, as: LinkedCTFResult<AnnotationConfirmReceipt>.self,
                generationToken: token) { [weak self] response in
            guard let self, self.annotationReviewSession == session,
                  self.annotationReviewStore == store, self.showingAnnotationReview else { return }
            self.annotationConfirmationBusy = false
            if let result = response?.result,
               result.state == "activated_read_only_overlay" || result.state == "revoked_projection" {
                self.status = "\(verb) verified for old #\(result.old_entry_id) → target #\(result.new_entry_id); old notes retained."
            } else {
                self.status = "\(verb) rejected; reread the conflict before another decision."
            }
            // A previous preview and its consent must never be reused.
            self.loadAnnotationReviewPage(offset: page)
        }
    }
    func chooseCatalog(create: Bool) {
        if create {
            let panel = NSSavePanel(); panel.title = "Create Atlas catalog"; panel.nameFieldStringValue = "Catalog.atlas.sqlite"
            panel.begin { [weak self] response in if response == .OK, let url = panel.url { self?.database = url.path; self?.initialize() } }
        } else {
            let panel = NSOpenPanel(); panel.title = "Open Atlas SQLite catalog"; panel.canChooseDirectories = false; panel.allowsMultipleSelection = false
            panel.begin { [weak self] response in if response == .OK, let url = panel.url { self?.database = url.path; self?.initialize() } }
        }
    }
    func verifyCatalog() {
        guard editableAllowed else { return }
        job = "maintenance"
        status = "Verifying catalog without modifying it..."
        request(["op": "validate_catalog"], as: EmptyReply.self) { [weak self] result in
            guard let self else { return }
            self.job = nil
            if result != nil { self.status = "Catalog verification passed. Original data unchanged." }
        }
    }
    func compactCopy() {
        guard editableAllowed else { return }
        let sourceDatabase = database
        let panel = NSSavePanel()
        panel.title = "Create a verified compact copy; original stays unchanged"
        panel.nameFieldStringValue = "Catalog-compact.atlas.sqlite"
        panel.begin { [weak self] response in
            guard let self, response == .OK, let url = panel.url,
                  self.database == sourceDatabase, self.mutationAllowed else { return }
            self.job = "maintenance"
            self.status = "Creating and verifying compact copy..."
            self.request(["op": "compact_catalog", "path": url.path], as: EmptyReply.self) { [weak self] result in
                guard let self else { return }
                self.job = nil
                if result != nil { self.status = "Compact copy verified. Use Open to view it; the original is unchanged." }
            }
        }
    }
    func mutate(_ args: [String: Any], label: String) {
        status = label
        request(args, as: EmptyReply.self) { [weak self] result in
            guard let self else { return }; self.job = nil
            if result != nil { self.refreshVolumes(); self.load() }
        }
    }
    func addDisk() {
        guard editableAllowed else { return }
        let panel = NSOpenPanel(); panel.title = "Choose a disk or folder to catalog"; panel.canChooseDirectories = true; panel.canChooseFiles = false
        panel.begin { [weak self] response in
            guard let self, response == .OK, let url = panel.url else { return }
            let identifier = "scan-" + UUID().uuidString; self.job = identifier
            self.mutate(["op": "scan", "root": url.path, "label": url.lastPathComponent, "job_id": identifier], label: "Scanning \(url.lastPathComponent)…")
        }
    }
    func cancel() {
        guard let job, canCancel else { return }
        request(["op": "cancel", "job_id": job], as: EmptyReply.self) { [weak self] _ in self?.status = "Cancellation requested…" }
    }
    func importCTF() {
        guard editableAllowed else { return }
        let panel = NSOpenPanel(); panel.title = "Import WhereIsIt CTF"; panel.canChooseDirectories = false
        panel.begin { [weak self] response in
            guard let self, response == .OK, let url = panel.url else { return }
            self.job = "import"; self.mutate(["op": "import_ctf", "path": url.path], label: "Importing CTF…")
        }
    }
    func export() {
        guard mutationAllowed else { return }
        guard !linkedMode || exportFormat != "ctf" else {
            status = "Linked CTF supports CSV/Parquet only; the original is read-only."
            return
        }
        let chosenFormat = exportFormat
        let originalDatabase = database
        let originalLinkedPath = linkedCTFPath
        let originalGeneration = linkedGeneration
        let panel = NSSavePanel()
        panel.title = chosenFormat == "ctf" ? "Export entire catalog as CTF" : (showingDuplicates ? "Export all matching duplicate candidates" : "Export current branch or query results")
        panel.nameFieldStringValue = "Catalog.\(chosenFormat)"
        panel.begin { [weak self] response in
            guard let self, response == .OK, let url = panel.url,
                  self.mutationAllowed, self.database == originalDatabase,
                  self.linkedCTFPath == originalLinkedPath,
                  self.linkedGeneration == originalGeneration else { return }
            var args: [String: Any]
            if let linked = self.linkedFields() {
                guard chosenFormat != "ctf" else { return }
                // Always create a new output: never overwrite another user file.
                args = ["op": "ctf_export_pinned", "output": url.path, "format": chosenFormat]
                args.merge(linked) { _, new in new }
            } else {
                args = ["op": "export_\(chosenFormat)", "path": url.path, "overwrite": true]
            }
            if chosenFormat != "ctf" {
                args["mode"] = self.showingDuplicates ? "duplicates" : (self.appliedQuery.isEmpty && self.volumeID != nil ? "children" : "search")
                args["query"] = self.appliedQuery
                args["duplicate_mode"] = self.appliedDuplicateMode
                if let volumeID = self.volumeID { args["volume_id"] = volumeID }
                if !self.showingDuplicates {
                    args["parent_id"] = self.stack.last.flatMap { $0.0 }.map { $0 as Any } ?? NSNull()
                }
                args["recursive"] = true
            }
            self.job = "export"
            if self.linkedMode {
                self.status = "Exporting verified linked CTF to new \(chosenFormat.uppercased())…"
                self.request(args, as: LinkedExportReceipt.self) { [weak self] result in
                    guard let self else { return }
                    self.job = nil
                    if let result {
                        self.status = "Exported \(result.result.rows) rows; original CTF unchanged."
                    }
                }
            } else {
                self.mutate(args, label: "Exporting \(chosenFormat.uppercased())…")
            }
        }
    }
}

@MainActor struct CatalogView: View {
    @StateObject private var model = Catalog()
    var body: some View {
        VStack(spacing: 4) {
            HStack(spacing: 6) {
                Button("Open…") { model.chooseCatalog(create: false) }.disabled(!model.mutationAllowed)
                Button("New…") { model.chooseCatalog(create: true) }.disabled(!model.mutationAllowed)
                Button("Check") { model.verifyCatalog() }.disabled(!model.editableAllowed)
                Button("Compact copy…") { model.compactCopy() }.disabled(!model.editableAllowed)
                Divider().frame(height: 16)
                Button("Add disk…") { model.addDisk() }.disabled(!model.editableAllowed)
                Menu("CTF") {
                    Button("Link read-only CTF…") { model.linkCTF() }
                    Button("Refresh linked CTF") { model.refreshLinkedCTF() }.disabled(!model.linkedMode)
                    Button("Review notes (read-only)…") { model.reviewAnnotations() }.disabled(!model.linkedMode)
                    Divider()
                    Button("Import CTF copy into SQLite…") { model.importCTF() }.disabled(!model.editableAllowed)
                }.disabled(!model.mutationAllowed)
                Picker("Export format", selection: $model.exportFormat) {
                    Text("CSV • current scope").tag("csv"); Text("Parquet • current scope").tag("parquet"); Text("CTF • entire catalog").tag("ctf")
                }.labelsHidden().frame(width: 174).accessibilityLabel("Export format and scope")
                Button("Export…") { model.export() }.disabled(!model.mutationAllowed || (model.linkedMode && (model.exportFormat == "ctf" || model.showingDuplicates)))
                Spacer(minLength: 4)
                if model.pending > 0 { ProgressView().controlSize(.small) }
                Button("Cancel scan") { model.cancel() }.disabled(!model.canCancel)
            }
            HStack(spacing: 6) {
                Button { model.back() } label: { Image(systemName: "chevron.left") }.help("Back to parent folder or leave results")
                Button { model.openSelected() } label: { Image(systemName: "folder") }.disabled(model.selected?.isFolder != true).keyboardShortcut(.downArrow, modifiers: [.command]).help("Open selected folder (Command-Down)")
                TextField("Search: name:film AND (ext:mkv OR ext:mp4) AND size:>1GB", text: $model.query).onSubmit { model.search() }.accessibilityLabel("Catalog search expression")
                Button("Search") { model.search() }.keyboardShortcut("f", modifiers: [.command])
                Picker("Duplicate matching", selection: $model.duplicateMode) {
                    Text("Name + size").tag("name_size"); Text("Size").tag("size"); Text("Name").tag("name")
                }.labelsHidden().frame(width: 120).accessibilityLabel("Duplicate matching mode")
                Button("Duplicates") { model.duplicates() }
            }
            HStack { Text("AND  OR  NOT  ( )   name:  path:  ext:  size:  category:").foregroundStyle(.secondary); Spacer(); Text(model.currentPath).lineLimit(1).truncationMode(.middle) }.font(.caption)
            HSplitView {
                VStack(alignment: .leading, spacing: 3) {
                    Text("DISKS").font(.caption.bold()).foregroundStyle(.secondary).padding(.horizontal, 6)
                    List(selection: Binding(get: { model.volumeID }, set: { model.volumeID = $0; model.selectVolume() })) {
                        Text("All disks / global search").tag(Optional<Int64>.none)
                        ForEach(model.volumes) { disk in
                            VStack(alignment: .leading, spacing: 1) {
                                Label(disk.label, systemImage: "externaldrive")
                                Text("\(disk.entry_count ?? 0) items • \(disk.root_path ?? "")").font(.caption).foregroundStyle(.secondary).lineLimit(1)
                            }.tag(Optional(disk.id)).help(disk.root_path ?? disk.label)
                        }
                    }.listStyle(.sidebar)
                    Divider()
                    Text("CATEGORIES").font(.caption.bold()).foregroundStyle(.secondary).padding(.horizontal, 6)
                    List(model.categories, selection: $model.categoryID) { category in Text(category.name).tag(category.id) }.listStyle(.sidebar)
                    HStack(spacing: 4) {
                        TextField("New category", text: $model.categoryName).onSubmit { model.createCategory() }.disabled(model.linkedMode)
                        Button("+") { model.createCategory() }.disabled(model.linkedMode || model.categoryName.trimmingCharacters(in: .whitespaces).isEmpty).help("Create category")
                    }.padding(.horizontal, 5)
                    HStack(spacing: 4) {
                        Button("Filter") { model.filterCategory() }.disabled(model.linkedMode || model.categoryID == nil)
                        Button("Assign selected") { model.assignCategory() }.disabled(model.linkedMode || model.selection == nil || model.categoryID == nil)
                    }.padding(.horizontal, 5)
                }.frame(minWidth: 170, idealWidth: 220, maxWidth: 320)
                Table(model.entries, selection: $model.selection) {
                    TableColumn("Name") { entry in Label(entry.name, systemImage: entry.isFolder ? "folder" : "doc").lineLimit(1) }.width(min: 160, ideal: 250)
                    TableColumn("Disk") { entry in Text(model.volumeLabel(entry.volume_id)) }.width(min: 100, ideal: 150)
                    TableColumn("Kind", value: \.kind).width(75)
                    TableColumn("Size") { Text($0.sizeText).monospacedDigit() }.width(95)
                    TableColumn("Modified") { Text($0.dateText) }.width(140)
                    TableColumn("Matches") { Text($0.duplicate_count.map(String.init) ?? "") }.width(65)
                    TableColumn("Path", value: \.relative_path).width(min: 160, ideal: 280)
                    TableColumn("Description") { Text($0.description ?? "") }.width(min: 120, ideal: 240)
                }.onTapGesture(count: 2) { model.openSelected() }
                .contextMenu { Button("Open folder") { model.openSelected() }.disabled(model.selected?.isFolder != true) }
            }
            HStack(spacing: 8) {
                Text(model.status).lineLimit(1).truncationMode(.middle).textSelection(.enabled)
                Spacer()
                Button("Previous") { model.offset = max(0, model.offset - model.limit); model.load() }.disabled(model.offset == 0)
                Button("Next") { model.offset += model.limit; model.load() }.disabled(!model.hasMore)
            }.font(.caption)
            Text(model.linkedMode ? "LINKED CTF (read-only) • " + (model.linkedCTFPath ?? "") : model.database).font(.caption2).foregroundStyle(.secondary).frame(maxWidth: .infinity, alignment: .leading).lineLimit(1).truncationMode(.middle)
        }
        .padding(7).controlSize(.small).frame(minWidth: 960, minHeight: 500)
        .sheet(isPresented: $model.showingAnnotationReview) {
            if let preview = model.annotationReview {
                VStack(alignment: .leading, spacing: 8) {
                    Text("CTF annotation conflicts").font(.headline)
                    Text("Read-only preview • \(preview.state) • \(preview.total) saved notes")
                        .font(.caption).foregroundStyle(.secondary)
                    if preview.state == "generation_conflict" {
                        Text("Rows \(preview.items.isEmpty ? 0 : model.annotationReviewOffset + 1)–\(model.annotationReviewOffset + preview.items.count) of \(preview.total)")
                            .font(.caption).monospacedDigit()
                        List(preview.items) { item in
                            HStack(spacing: 10) {
                                Text("Old #\(item.old_entry_id)").monospacedDigit().frame(width: 90, alignment: .leading)
                                Text(item.candidate_state).frame(width: 136, alignment: .leading)
                                Text("Candidates: \(item.candidate_count_lower_bound)\(item.candidates_truncated ? "+" : "")")
                                    .frame(width: 112, alignment: .leading)
                                Text(item.conflictText).foregroundStyle(.secondary)
                                Spacer(minLength: 2)
                                if item.canSelect {
                                    Button(model.annotationSelectedOldID == item.old_entry_id ? "Selected" : "Choose…") {
                                        model.selectAnnotation(item)
                                    }.disabled(model.annotationReviewLoading || model.annotationConfirmationBusy)
                                }
                                if item.canActivate, let target = item.confirmed_selection?.new_entry_id {
                                    Button("Activate…") {
                                        model.proposeAnnotationOverlay(.activation(oldID: item.old_entry_id, newID: target))
                                    }.disabled(model.annotationReviewLoading || model.annotationConfirmationBusy)
                                }
                                if item.canRevoke, let target = item.activation_target_entry_id {
                                    Button("Revoke…") {
                                        model.proposeAnnotationOverlay(.revocation(oldID: item.old_entry_id, newID: target))
                                    }.disabled(model.annotationReviewLoading || model.annotationConfirmationBusy)
                                }
                            }.font(.caption)
                        }
                        if let selected = model.selectedAnnotationRow {
                            VStack(alignment: .leading, spacing: 5) {
                                Text("Review old #\(selected.old_entry_id) • \(selected.candidate_state)")
                                    .font(.subheadline).bold()
                                HStack {
                                    Picker("Target entry", selection: $model.annotationSelectedCandidateID) {
                                        Text("Choose a destination explicitly").tag(nil as Int64?)
                                        ForEach(selected.candidates) { candidate in
                                            Text("Entry #\(candidate.entry_id) • disk #\(candidate.volume_id)")
                                                .tag(candidate.entry_id as Int64?)
                                        }
                                    }.pickerStyle(.menu)
                                    Button("Confirm mapping…") {
                                        model.annotationConsent = .mapping
                                    }.disabled(!model.canConfirmAnnotation)
                                }
                                Text("Structural metadata is not file-content identity. Confirmation records a choice only, without activating notes.")
                                    .font(.caption).foregroundStyle(.secondary)
                            }
                        }
                        Text("20 records per freshly SHA-verified page. Mapping, activation and revocation require separate consent; journal state does not prove a stale note is valid.")
                            .font(.caption)
                    } else {
                        Text(preview.state == "current_generation"
                             ? "Notes already match this generation; no reconciliation required."
                             : "No initialized user annotation store was found.")
                    }
                    Text("The CTF and derived cache stay read-only. Only an explicitly approved mapping is written to the separate notes database.")
                        .font(.caption).foregroundStyle(.secondary)
                    HStack {
                        Button("Previous") {
                            model.loadAnnotationReviewPage(offset: max(0, model.annotationReviewOffset - model.annotationPageSize))
                        }.disabled(!model.annotationReviewHasPrevious || model.annotationReviewLoading || model.annotationConfirmationBusy)
                        Button("Next") {
                            model.loadAnnotationReviewPage(offset: model.annotationReviewOffset + model.annotationPageSize)
                        }.disabled(!model.annotationReviewHasNext || model.annotationReviewLoading || model.annotationConfirmationBusy)
                        Spacer()
                        Button("Close") { model.resetAnnotationReview() }
                            .keyboardShortcut(.cancelAction)
                            .disabled(model.annotationConfirmationBusy)
                    }
                }
                .padding(14).frame(width: 820, height: 520)
                .confirmationDialog("Approve this annotation action?",
                                    isPresented: Binding(
                                        get: { model.annotationConsent != nil },
                                        set: { if !$0 { model.annotationConsent = nil } }),
                                    titleVisibility: .visible) {
                    if model.annotationConsent == .mapping {
                        Button("Confirm mapping (leave inactive)") {
                            model.confirmAnnotationSelection()
                        }.disabled(!model.canConfirmAnnotation)
                    }
                    if case .some(.activation(let oldID, let newID)) = model.annotationConsent {
                        Button("Activate read-only overlay") {
                            model.applyAnnotationOverlay(.activation(oldID: oldID, newID: newID))
                        }
                    }
                    if case .some(.revocation(let oldID, let newID)) = model.annotationConsent {
                        Button("Revoke active overlay", role: .destructive) {
                            model.applyAnnotationOverlay(.revocation(oldID: oldID, newID: newID))
                        }
                    }
                    Button("Cancel", role: .cancel) {}
                } message: {
                    if model.annotationConsent == .mapping {
                        Text("Save the chosen structural mapping only. Notes remain inactive. No CTF or cache writes.")
                    } else if case .some(.activation) = model.annotationConsent {
                        Text("Independently approve a read-only projection. Source SHA, note revision and document are rechecked; original notes remain.")
                    } else {
                        Text("Revoke only the chosen active projection. The original notes and private audit remain; revoked mappings cannot be replayed.")
                    }
                }
            } else {
                VStack(spacing: 12) {
                    if model.annotationReviewLoading { ProgressView("Verifying read-only page…") }
                    else { Text("Preview could not be loaded. No notes were changed.") }
                    Button("Close") { model.resetAnnotationReview() }
                        .keyboardShortcut(.cancelAction)
                }.padding(20).frame(width: 500, height: 160)
            }
        }
        .alert("WhereIsIt AI", isPresented: Binding(get: { model.error != nil }, set: { if !$0 { model.error = nil } })) { Button("OK") { model.error = nil } } message: { Text(model.error ?? "") }
    }
}

@main @MainActor struct AtlasCatalogApp: App {
    var body: some Scene {
        WindowGroup("WhereIsIt AI") { CatalogView() }
        .defaultSize(width: 1200, height: 740)
        .commands { CommandGroup(replacing: .newItem) {} }
    }
}
