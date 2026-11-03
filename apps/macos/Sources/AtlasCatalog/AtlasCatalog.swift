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
struct CandidateDiskGroup: Decodable, Identifiable {
    let volume_id: Int64
    let label: String
    let count: Int
    var id: Int64 { volume_id }
}
struct EntryPage: Decodable {
    let items: [Entry]
    let total: Int?
    let offset: Int?
    let limit: Int?
    let has_more: Bool?
    let distinct_disks: Int?
    let all_total: Int?
    let disk_groups: [CandidateDiskGroup]?
}
struct CoreError: Decodable { let code: String?; let message: String }
struct Envelope<T: Decodable>: Decodable { let ok: Bool; let data: T?; let error: CoreError? }
struct EmptyReply: Decodable {}
struct Category: Decodable, Identifiable { let id: Int64; let parent_id: Int64?; let name: String }
struct CreatedCategory: Decodable { let id: Int64 }
struct LinkedCTFSync: Decodable { let generation: String; let entries: Int64; let volumes: Int64; let rebuilt: Bool }

// File system UUIDs are from the currently mounted OS volume, never a
// historical CTF path. A cloned disk may share a UUID: require human pairing,
// detect simultaneous collisions, and never infer byte-equivalent content.
struct LivePhysicalVolume: Identifiable, Hashable {
    let id: String
    let name: String
    let currentMount: String
}
struct PhysicalStateRow: Decodable {
    let volume_id: Int64
    let state: String
    let verified_physical_id: String?
}
struct PhysicalStateReply: Decodable {
    let items: [PhysicalStateRow]
    let probe_ok: Bool
}
struct PhysicalPairReply: Decodable { let paired: Bool }
struct PhysicalUnpairReply: Decodable { let removed: Bool }
struct ShelfLabelRow: Decodable { let volume_id: Int64; let shelf_label: String }
struct ShelfCollisionGroup: Decodable { let volume_ids: [Int64] }
struct ShelfLabelReply: Decodable {
    let items: [ShelfLabelRow]
    let collisions: [ShelfCollisionGroup]?
}
struct ShelfLabelSaved: Decodable { let saved: Bool; let volume_id: Int64; let shelf_label: String }

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
// Only freshly verified results from ctf_annotations_activated_list_pinned
// may appear here. Historical 'active' audit status is not an overlay.
struct VerifiedAnnotation: Decodable, Identifiable {
    let entry_id: Int64
    let old_entry_id: Int64
    let revision: Int64
    let note: String
    let tags: String
    let read_only: Bool
    let activation: String
    var id: Int64 { old_entry_id }
}
struct VerifiedOverlayPage: Decodable {
    let state: String
    let total: Int64
    let conflicts: Int64
    let items: [VerifiedAnnotation]
    let offset: Int64?
    let limit: Int64?
    let read_only_source: Bool?
    let unresolved_unconfirmed_annotations_not_projected: Bool?
    func isDisplaySafe(expectedOffset: Int) -> Bool {
        guard expectedOffset >= 0 else { return false }
        if state == "uninitialized" || state == "no_cross_generation_activations" {
            return total == 0 && conflicts == 0 && items.isEmpty
        }
        return (state == "partial_activation" || state == "activation_conflict")
            && offset == Int64(expectedOffset) && limit == 20
            && read_only_source == true
            && unresolved_unconfirmed_annotations_not_projected == true
            && total >= 0 && conflicts >= 0 && items.count <= 20
            && items.allSatisfy {
                $0.read_only && $0.activation == "explicit_reviewed_projection"
                && $0.entry_id > 0 && $0.old_entry_id > 0 && $0.revision > 0
            }
    }
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

private struct ReturnSelection {
    let id: Int64
    let diskID: Int64
}

private struct CopyReturnContext {
    let volumeID: Int64?
    let stack: [(Int64?, String)]
    let query: String
    let appliedQuery: String
    let offset: Int
    let showingDuplicates: Bool
    let appliedDuplicateMode: String
    let selectedFile: ReturnSelection
}

// Candidate-folder excursions retain the original source-search history.
private struct CandidateFolderReturnContext {
    let sourceID: Int64
    let sourceName: String?
    let focusVolumeID: Int64?
    let offset: Int
    let diskFilter: String
    let selectedFile: ReturnSelection
}

@MainActor final class Catalog: ObservableObject {
    @Published var database: String
    @Published var volumes: [Volume] = []
    @Published var diskFilter = ""
    // UI-only preference; never persisted with CTF, source paths or physical pairings.
    @Published var registerOrder = UserDefaults.standard.string(forKey: "AtlasCatalog.registerOrder") ?? "catalog"
    var registerOrderCaption: String {
        switch registerOrder {
        case "shelf": return "Shelf A–Z"
        case "os": return "Last OS state"
        default: return "Catalog order"
        }
    }
    func cycleRegisterOrder() {
        switch registerOrder {
        case "catalog": registerOrder = "shelf"
        case "shelf": registerOrder = "os"
        default: registerOrder = "catalog"
        }
        UserDefaults.standard.set(registerOrder, forKey: "AtlasCatalog.registerOrder")
        status = "Disk register: \(registerOrderCaption). Candidate copies keep their separate priority; global search and physical pairing unchanged."
    }
    @Published var volumeID: Int64?
    @Published var physicalStates: [Int64: String] = [:]
    @Published var verifiedPhysicalIDs: [Int64: String] = [:]
    private var pendingMountCopy: (disk: Int64, scope: String, openInFinder: Bool)?
    private var mountActionGeneration = 0
    @Published var shelfLabels: [Int64: String] = [:]
    @Published var shelfTagConflicts: [Int64: [Int64]] = [:]
    @Published var mountedPhysicalVolumes: [LivePhysicalVolume] = []
    @Published var selectedPhysicalID = ""
    @Published var physicalProbeOK = false
    // Monotonic clock avoids a stale connected badge after suspend.
    private var physicalVerifiedUptime: TimeInterval?
    private let physicalStateTTL: TimeInterval = 12
    private var physicalCheckGeneration = 0
    // Separate from periodic probes: mount events revoke outstanding CTF
    // source checks that would otherwise restart a stale Copy OS root command.
    private var physicalMountEventGeneration = 0
    private var shelfLabelRequestGeneration = 0
    @Published var entries: [Entry] = []
    @Published var pageSortColumn = "natural"
    @Published var pageSortDescending = false
    @Published var selection: Int64?
    @Published var restoredSelectionNonce = 0
    private var pendingReturnSelection: (token: Int, row: ReturnSelection)?
    // Fresh page focus is requested by a paging action, never a stale response.
    private var pendingPageFocusToken: Int?
    @Published var query = ""
    // Only actual edits from the native TextField may arm live filename search.
    // Programmatic Back, copy navigation and scoped/duplicate operations must
    // never schedule a second global search 450 ms later.
    @Published private(set) var liveFilenameTaskID: UInt64 = 0
    private var pendingLiveFilenameQuery: String?
    func userEditedSearchText(_ value: String) {
        query = value
        pendingLiveFilenameQuery = Self.liveFilenameSearchQuery(value)
        liveFilenameTaskID &+= 1
    }
    private func cancelPendingLiveFilenameSearch() {
        pendingLiveFilenameQuery = nil
        liveFilenameTaskID &+= 1
    }
    func pendingLiveFilenameSearch(for token: UInt64) -> String? {
        guard token == liveFilenameTaskID else { return nil }
        return pendingLiveFilenameQuery
    }
    // Plain filename typeahead; partially typed Boolean and field queries
    // must wait for an explicit Return and never rewrite the query.
    static func liveFilenameSearchQuery(_ raw: String) -> String? {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.count >= 2, trimmed.utf8.count <= 256 else { return nil }
        let accepted = CharacterSet.alphanumerics
            .union(.whitespacesAndNewlines)
            .union(CharacterSet(charactersIn: "._-'"))
        guard trimmed.unicodeScalars.allSatisfy({ accepted.contains($0) }) else { return nil }
        let operators: Set<String> = ["AND", "OR", "NOT"]
        guard !trimmed.split(whereSeparator: { $0.isWhitespace })
            .contains(where: { operators.contains($0.uppercased()) }) else { return nil }
        return trimmed
    }
    func runLiveFilenameSearchIfCurrent(_ expected: String, editToken: UInt64) {
        // The text can remain identical after a manual This disk/Duplicates/Back.
        // Only the still-pending user-edit token may switch scope to all disks.
        guard pendingLiveFilenameSearch(for: editToken) == expected,
              Self.liveFilenameSearchQuery(query) == expected else { return }
        if appliedQuery == expected && !showingDuplicates && volumeID == nil && copySourceID == nil {
            cancelPendingLiveFilenameSearch()
            return
        }
        cancelPendingLiveFilenameSearch()
        // Search/Return already supplies all-disk scope, bounded pages,
        // generation checks and original linked CTF SHA re-verification.
        search()
    }
    // This preset uses catalogued extensions; no movie identification or hash.
    var videoFilterActive: Bool {
        let current = query.trimmingCharacters(in: .whitespacesAndNewlines)
        if current == "type:video" { return true }
        let suffix = " AND type:video"
        guard current.hasSuffix(suffix) else { return false }
        let wrapped = String(current.dropLast(suffix.count))
        return wrapped.hasPrefix("(") && wrapped.hasSuffix(")")
    }
    func toggleVideoFormatSearch() {
        let current = query.trimmingCharacters(in: .whitespacesAndNewlines)
        let next: String
        if current == "type:video" {
            next = ""
        } else if videoFilterActive {
            let wrapped = String(current.dropLast(" AND type:video".count))
            next = String(wrapped.dropFirst().dropLast())
        } else if current.isEmpty {
            next = "type:video"
        } else {
            next = "(\(current)) AND type:video"
        }
        guard next.utf8.count <= 4096 else {
            status = "Query would exceed 4096 UTF-8 bytes. Shorten it first."
            return
        }
        query = next
        search()
    }
    // Last 128 accepted GLOBAL query pages, local-only and in-memory.
    // No source SHA, filename, query, physical disk ID or telemetry is retained.
    private var globalSearchLatencyMs: [Int] = []
    private let globalSearchLatencyWindow = 128
    private let globalSearchLatencyMinimum = 30
    private func recordGlobalSearchLatency(_ ms: Int) {
        guard pageReady, !appliedQuery.isEmpty, volumeID == nil,
              !showingDuplicates, copySourceID == nil else { return }
        if globalSearchLatencyMs.count == globalSearchLatencyWindow {
            globalSearchLatencyMs.removeFirst()
        }
        globalSearchLatencyMs.append(max(0, ms))
    }
    func resetGlobalSearchLatency() {
        globalSearchLatencyMs.removeAll()
        status = "Local search latency session cleared; source unchanged."
    }
    func showGlobalSearchLatency() {
        let count = globalSearchLatencyMs.count
        guard count >= globalSearchLatencyMinimum else {
            status = "Global search: \(count)/30 fresh verified pages; no p95 yet. Local-only diagnostic."
            return
        }
        let sorted = globalSearchLatencyMs.sorted()
        let p50 = sorted[((count * 50 + 99) / 100) - 1]
        let p95 = sorted[((count * 95 + 99) / 100) - 1]
        status = "Global search: \(count) fresh pages; session p50=\(p50) ms, p95=\(p95) ms (nearest-rank, last 128). NOT hardware/release acceptance."
    }

    var appliedQuery = ""
    @Published var status = "Opening catalog…"
    @Published var error: String?
    @Published var pending = 0
    @Published var job: String?
    @Published var duplicateMode = "name_size"
    var appliedDuplicateMode = "name_size"
    @Published var showingDuplicates = false
    @Published var copySourceID: Int64? {
        didSet {
            if copySourceID == nil
                && diskFilter.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() == "copies:" {
                diskFilter = ""
            }
        }
    }
    @Published var copySourceName: String?
    @Published var copyDiskGroups: [CandidateDiskGroup] = []
    @Published var pageReady = false
    @Published var awaitingPage = false
    @Published var retryPageAvailable = false
    @Published var copyFocusVolumeID: Int64?
    private var copyReturn: CopyReturnContext?
    private var candidateFolderReturn: CandidateFolderReturnContext?
    // Both entry ID and catalog generation must remain unchanged during the OS probe.
    private var pendingConnectedCopyFocus: (source: Int64, scope: String)?
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
    @Published var showingVerifiedOverlay = false
    @Published var verifiedOverlay: VerifiedOverlayPage?
    @Published var verifiedOverlayOffset = 0
    @Published var verifiedOverlayLoading = false
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
    var verifiedOverlayHasPrevious: Bool {
        verifiedOverlay?.isDisplaySafe(expectedOffset: verifiedOverlayOffset) == true && verifiedOverlayOffset > 0
    }
    var verifiedOverlayHasNext: Bool {
        guard let page = verifiedOverlay, page.isDisplaySafe(expectedOffset: verifiedOverlayOffset), !page.items.isEmpty else { return false }
        return Int64(verifiedOverlayOffset + page.items.count) < page.total
    }
    var linkedGeneration: String?
    var linkedMode: Bool { linkedCTFPath != nil } // Read-only even when generation is missing.
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
    private var pageRequestStartedUptime: TimeInterval?
    let limit = 250
    var currentPath: String { stack.last?.1 ?? "/" }
    var canPreviousResultsPage: Bool { pageReady && !awaitingPage && offset > 0 }
    var canNextResultsPage: Bool {
        pageReady && !awaitingPage && hasMore && offset <= Int.max - limit
    }
    func moveResultsPage(forward: Bool) {
        guard forward ? canNextResultsPage : canPreviousResultsPage else {
            status = "Page navigation unavailable until the current page is verified."
            return
        }
        offset = forward ? offset + limit : max(0, offset - limit)
        pendingPageFocusToken = generation + 1
        load() // Always reverify linked original CTF generation on every page.
    }
    var mutationAllowed: Bool { job == nil && pending == 0 }
    var canCancel: Bool { job?.hasPrefix("scan-") == true }
    var selected: Entry? { entries.first { $0.id == selection } }

    // Presentation only: sort at most one 250-row page, never the database/CTF.
    private func optionalSortNumber(_ a: Int64?, _ b: Int64?) -> Int {
        switch (a, b) {
        case (nil, nil): return 0
        case (nil, _): return 2
        case (_, nil): return -2
        case let (.some(left), .some(right)):
            return left < right ? -1 : (left > right ? 1 : 0)
        }
    }

    func focusCurrentPageResults() -> Bool {
        guard pageReady, let first = displayEntries.first else {
            status = "No verified current result page to focus."
            return false
        }
        if selected == nil || !entries.contains(where: { $0.id == selection }) {
            selection = first.id
        }
        return true
    }

    var displayEntries: [Entry] {
        guard pageSortColumn != "natural" else { return entries }
        return entries.sorted { left, right in
            let comparison: Int
            switch pageSortColumn {
            case "name":
                comparison = left.name.localizedStandardCompare(right.name).rawValue
            case "disk":
                comparison = volumeLabel(left.volume_id).localizedStandardCompare(volumeLabel(right.volume_id)).rawValue
            case "kind":
                comparison = left.kind.localizedStandardCompare(right.kind).rawValue
            case "size":
                comparison = optionalSortNumber(left.size, right.size)
            case "modified":
                comparison = optionalSortNumber(left.modified_unix, right.modified_unix)
            case "candidates":
                comparison = optionalSortNumber(left.duplicate_count, right.duplicate_count)
            case "path":
                comparison = left.relative_path.localizedStandardCompare(right.relative_path).rawValue
            default:
                comparison = 0
            }
            if abs(comparison) == 2 { return comparison < 0 } // unknown metadata last
            if comparison == 0 { return left.id < right.id }
            return pageSortDescending ? comparison > 0 : comparison < 0
        }
    }

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
        globalSearchLatencyMs.removeAll()
        linkedCTFPath = nil; linkedGeneration = nil
        resetAnnotationReview()
        generation += 1; candidateClipboardRequestSerial += 1; volumeID = nil; stack = [(nil, "/")]; entries = []; query = ""; appliedQuery = ""; showingDuplicates = false; offset = 0
        copySourceID = nil; copySourceName = nil; copyReturn = nil; candidateFolderReturn = nil
        volumes = []; diskFilter = ""; categories = []; categoryID = nil; selection = nil; total = 0; hasMore = false
        physicalStates = [:]; verifiedPhysicalIDs = [:]; pendingMountCopy = nil; mountActionGeneration += 1
        shelfLabels = [:]; mountedPhysicalVolumes = []; selectedPhysicalID = ""; physicalProbeOK = false
        physicalCheckGeneration += 1
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

    // The row status is the existing fresh OS+service verified pairing, not
    // a catalog path, shelf label, file-existence or content-hash check.
    func resultDiskOSState(_ disk: Int64) -> String {
        let state = physicalState(disk)
        if state == "connected" && (verifiedPhysicalIDs[disk]?.isEmpty ?? true) {
            return "unverified"
        }
        return state
    }
    func resultDiskOSMarker(_ disk: Int64) -> String {
        switch resultDiskOSState(disk) {
        case "connected": return "[OS+]"
        case "offline": return "[OFF]"
        case "conflict": return "[ID!]"
        default: return "[?]"
        }
    }
    func resultDiskOSAccessibility(_ disk: Int64) -> String {
        switch resultDiskOSState(disk) {
        case "connected": return "Current OS filesystem UUID matched explicit disk pairing. Catalogued file bytes are NOT verified."
        case "offline": return "Paired filesystem UUID was absent from a complete OS scan. Catalog remains searchable offline."
        case "conflict": return "Ambiguous physical filesystem identity. A historical label does not resolve the conflict."
        default: return "Current physical connection not verified."
        }
    }

    // The ID-first main-table caption stays unambiguous when the locator is
    // hidden. Private shelf tags and historical labels are NOT USB identities.
    func resultDiskCaption(_ disk: Int64) -> String {
        guard disk > 0, volumes.contains(where: { $0.id == disk }) else {
            return "UNKNOWN CATALOG DISK"
        }
        let label = volumeLabel(disk).replacingOccurrences(of: "\n", with: " ")
            .replacingOccurrences(of: "\r", with: " ")
            .replacingOccurrences(of: "\t", with: " ")
        let sticker = shelfLabel(disk).replacingOccurrences(of: "\n", with: " ")
            .replacingOccurrences(of: "\r", with: " ")
            .replacingOccurrences(of: "\t", with: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let shared = shelfConflictCaption(disk) == nil ? "" : " [SHARED STICKER]"
        let badge = resultDiskOSMarker(disk)
        return sticker.isEmpty ? "#\(disk) \(badge) • \(label)" : "#\(disk) \(badge) • \(sticker)\(shared) • \(label)"
    }
    func resultDiskAccessibility(_ disk: Int64) -> String {
        guard disk > 0, volumes.contains(where: { $0.id == disk }) else {
            return "Unknown catalog disk. Physical identity not verified."
        }
        let sticker = shelfLabel(disk)
        return "Catalog disk ID \(disk). Recorded label: \(volumeLabel(disk)). "
            + "Owner sticker or shelf: \(sticker.isEmpty ? "not assigned" : sticker). "
            + "\(shelfConflictCaption(disk) ?? "") \(resultDiskOSAccessibility(disk)) "
            + "Catalog metadata only; physical pairing is independent of file contents."
    }

    // View-only filtering: never change volumeID, query scope or disk pairing.
    // Page register is a view of already-fetched SHA-verified rows only.
    var currentPageDiskCounts: [Int64: Int] {
        guard pageReady, !awaitingPage else { return [:] }
        let registered = Set(volumes.map(\.id))
        var counts: [Int64: Int] = [:]
        for entry in entries where registered.contains(entry.volume_id) {
            counts[entry.volume_id, default: 0] += 1
        }
        return counts
    }
    var isPageRegisterMode: Bool {
        diskFilter.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() == "page:"
    }
    func pageRegisterCount(_ disk: Int64) -> Int { currentPageDiskCounts[disk] ?? 0 }
    func pageRegisterAccessibility(_ disk: Volume) -> String {
        "Catalog disk number \(disk.id), \(disk.label). \(pageRegisterCount(disk.id)) hits on CURRENT verified page only (maximum 250). Owner sticker \(shelfLabel(disk.id).isEmpty ? "not assigned" : shelfLabel(disk.id)). Last OS check: \(physicalCaption(disk.id)). \(shelfConflictCaption(disk.id) ?? ""). Unloaded pages and file contents unverified."
    }
    func revealCurrentPageDisksInRegister() {
        guard pageReady, !awaitingPage, !entries.isEmpty else {
            status = "Wait for a verified nonempty results page; no stale disk membership."
            return
        }
        diskFilter = "page:"
        status = "CURRENT PAGE ONLY: exact disk IDs and counts from loaded results. Search, selected file and OS pairing unchanged."
    }
    private var filteredVolumes: [Volume] {
        let needle = diskFilter.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !needle.isEmpty else { return volumes }
        // copies: is a temporary, view-only exact ID match from the active
        // cross-disk metadata candidate groups, never OS mount evidence.
        if needle.lowercased() == "page:" {
            let counts = currentPageDiskCounts
            return volumes.filter { (counts[$0.id] ?? 0) > 0 }.sorted { lhs, rhs in
                let l = counts[lhs.id] ?? 0, r = counts[rhs.id] ?? 0
                return l == r ? lhs.id < rhs.id : l > r
            }
        }
        if needle.lowercased() == "copies:" {
            guard copySourceID != nil else { return [] }
            // Preserve priority order used by the pickup list, but keep
            // actual catalog Volume objects and exact IDs for selection.
            var seen = Set<Int64>()
            return orderedCopyDiskGroups.compactMap { group -> Volume? in
                guard group.count > 0, seen.insert(group.volume_id).inserted else { return nil }
                return volumes.first { $0.id == group.volume_id }
            }
        }
        // id:42 uses strict catalog identity, not a fuzzy number/label match.
        // This filter never changes search scope or verifies a mounted USB.
        if needle.lowercased().hasPrefix("id:") {
            guard let requestedID = Int64(String(needle.dropFirst(3))), requestedID > 0
            else { return [] }
            return volumes.filter { $0.id == requestedID }
        }
        // Reserved audit filters match exact catalog IDs only; failed or
        // pending native OS probes downgrade everything to unverified.
        let lower = needle.lowercased()
        if lower.hasPrefix("state:") {
            let requested = String(lower.dropFirst(6)).trimmingCharacters(in: .whitespaces)
            guard ["connected", "offline", "conflict", "unverified"].contains(requested)
            else { return [] }
            return volumes.filter { physicalState($0.id) == requested }
        }
        if lower.hasPrefix("tag:") {
            let requested = String(lower.dropFirst(4)).trimmingCharacters(in: .whitespaces)
            switch requested {
            case "missing":
                return volumes.filter { shelfLabel($0.id).trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
            case "shared":
                return volumes.filter { disk in
                    shelfTagConflicts[disk.id]?.contains(where: { $0 != disk.id }) ?? false
                }
            default:
                return []
            }
        }
        return volumes.filter {
            $0.label.localizedStandardContains(needle) || String($0.id).contains(needle)
                || (shelfLabels[$0.id] ?? "").localizedStandardContains(needle)
        }
    }
    // This is presentation order ONLY. Exact catalog IDs remain List tags,
    // the selected browse scope is untouched, and candidate-only mode retains
    // its connected/count ordering rather than the user's ordinary register order.
    var visibleVolumes: [Volume] {
        let rows = filteredVolumes
        guard !isCandidateRegisterMode && !isPageRegisterMode else { return rows }
        switch registerOrder {
        case "shelf":
            return rows.sorted { lhs, rhs in
                let a = shelfLabel(lhs.id).trimmingCharacters(in: .whitespacesAndNewlines)
                let b = shelfLabel(rhs.id).trimmingCharacters(in: .whitespacesAndNewlines)
                if a.isEmpty != b.isEmpty { return !a.isEmpty }
                let comparison = a.localizedStandardCompare(b)
                return comparison == .orderedSame ? lhs.id < rhs.id : comparison == .orderedAscending
            }
        case "os":
            func rank(_ disk: Int64) -> Int {
                switch physicalState(disk) {
                case "connected": return 0
                case "offline": return 1
                case "conflict": return 2
                default: return 3
                }
            }
            return rows.sorted { lhs, rhs in
                let left = rank(lhs.id), right = rank(rhs.id)
                return left == right ? lhs.id < rhs.id : left < right
            }
        default: return rows
        }
    }
    var isCandidateRegisterMode: Bool {
        diskFilter.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() == "copies:"
            && copySourceID != nil
    }
    func candidateRegisterCount(_ disk: Int64) -> Int {
        copyDiskGroups.filter { $0.volume_id == disk }.map(\.count).max() ?? 0
    }
    func candidateRegisterAccessibility(_ disk: Volume) -> String {
        let tag = shelfLabel(disk.id)
        return "Catalog disk number \(disk.id), \(disk.label). \(candidateRegisterCount(disk.id)) recorded name and size metadata candidates, not verified file copies. Owner sticker or shelf \(tag.isEmpty ? "not assigned" : tag). Last OS check: \(candidateCopyOSCaption(disk.id)). \(shelfConflictCaption(disk.id) ?? ""). File contents not compared."
    }
    var registerAuditCounts: (connected: Int, offline: Int, conflict: Int, unknown: Int, missing: Int, shared: Int) {
        var counts = (connected: 0, offline: 0, conflict: 0, unknown: 0, missing: 0, shared: 0)
        for disk in volumes {
            switch physicalState(disk.id) {
            case "connected": counts.connected += 1
            case "offline": counts.offline += 1
            case "conflict": counts.conflict += 1
            default: counts.unknown += 1
            }
            if shelfLabel(disk.id).trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                counts.missing += 1
            }
            if shelfTagConflicts[disk.id]?.contains(where: { $0 != disk.id }) == true {
                counts.shared += 1
            }
        }
        return counts
    }
    var registerAuditCaption: String {
        let c = registerAuditCounts
        return "OS ✓\(c.connected) ○\(c.offline) !\(c.conflict) ?\(c.unknown) • no tag \(c.missing) shared \(c.shared)"
    }
    var registerAuditAccessibility: String {
        let c = registerAuditCounts
        return "Last OS probe: \(c.connected) explicitly paired and OS-verified, \(c.offline) paired but absent, \(c.conflict) identity conflicts, \(c.unknown) not verified. \(c.missing) missing owner sticker and \(c.shared) disks using a shared sticker. No file contents checked."
    }
    func showPhysicalStateInRegister(_ state: String) {
        guard state == "connected" || state == "unverified" else { return }
        diskFilter = "state:" + state
        refreshPhysicalPresence() // asynchronous: old positive badges cleared immediately
        status = "Filtering catalog disk register by a fresh OS UUID check. Search and file selection unchanged; no file contents verified."
    }
    func revealCopyCandidateDisksInRegister() {
        guard copySourceID != nil, !copyDiskGroups.isEmpty else {
            status = "Find metadata copies of a file before filtering the disk register."
            return
        }
        diskFilter = "copies:"
        status = "Showing exact candidate catalog disk IDs in the register. Search, selected file and OS pairing remain unchanged."
    }
    func revealSelectedDiskInRegister() {
        guard let entry = selected,
              volumes.contains(where: { $0.id == entry.volume_id }) else {
            status = "Select a current file result with a registered catalog disk."
            return
        }
        // View-only change: leave global/copy query, page and selected entry
        // intact; only a separate explicit disk click begins browsing.
        diskFilter = "id:\(entry.volume_id)"
        status = "Showing exact catalog disk #\(entry.volume_id) in register. Search scope and OS pairing unchanged; click disk to browse."
    }
    var selectedDiskForPairing: Int64? { selected?.volume_id ?? volumeID }
    var physicalUserStore: String {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("AtlasCatalog/physical-volume-bindings.sqlite").path
    }
    var physicalScope: String {
        if let linkedGeneration { return "ctf:" + linkedGeneration }
        return "sqlite:" + database
    }
    // Keep raw OS observations for the shared conflict detector; the picker
    // only displays UUIDs observed exactly once in the current scan.
    var pairablePhysicalVolumes: [LivePhysicalVolume] {
        let counts = Dictionary(mountedPhysicalVolumes.map { ($0.id, 1) }, uniquingKeysWith: +)
        return mountedPhysicalVolumes.filter { counts[$0.id] == 1 }
            .sorted {
                let order = $0.name.localizedStandardCompare($1.name)
                return order == .orderedSame ? $0.currentMount < $1.currentMount : order == .orderedAscending
            }
    }
    func shelfLabel(_ disk: Int64) -> String { shelfLabels[disk] ?? "" }
    // Canonical, private per-selection text for the inspector AND the
    // context menu. Never infer current file contents from catalog metadata.
    func retrievalNote(for entry: Entry) -> String {
        let shelf = self.shelfLabel(entry.volume_id)
        let size = entry.size.map { "\($0) recorded bytes" } ?? "unknown"
        let candidates = entry.duplicate_count.map { "\($0) metadata candidates" } ?? "not calculated"
        return [
            "WhereIsIt AI — catalog retrieval note",
            "Disk: \(self.volumeLabel(entry.volume_id))",
            "Catalog disk ID: \(entry.volume_id)",
            "Owner sticker / shelf: \(shelf.isEmpty ? "not assigned" : shelf)",
            "File: \(entry.name)",
            "Recorded relative path: \(entry.relative_path)",
            "Recorded size: \(size)",
            "Physical: \(self.physicalCaption(entry.volume_id))",
            "Copies: \(candidates); file contents not compared",
            "OS state is from the last probe and may change; catalog paths are historical.",
        ].joined(separator: "\n")
    }

    func shelfConflictCaption(_ disk: Int64) -> String? {
        guard let members = shelfTagConflicts[disk] else { return nil }
        let others = members.filter { $0 != disk }.map { "#\($0)" }
        guard !others.isEmpty else { return nil }
        return "SHARED STICKER also on \(others.joined(separator: ", ")). Check actual physical labels; this is NOT USB identity."
    }

    func refreshShelfLabels() {
        shelfLabelRequestGeneration += 1
        let requestToken = shelfLabelRequestGeneration
        let scope = physicalScope
        let catalogIDs = Set(volumes.map(\.id))
        // A previous catalog's tag must not remain visible while loading another.
        shelfLabels = [:]
        shelfTagConflicts = [:]
        var args = physicalArguments([], probeOK: false)
        args["op"] = "physical_shelf_labels"
        request(args, as: ShelfLabelReply.self) { [weak self] result in
            guard let self, self.physicalScope == scope,
                  self.shelfLabelRequestGeneration == requestToken else { return }
            self.shelfLabels = Dictionary(uniqueKeysWithValues:
                (result?.items ?? []).filter { row in
                    catalogIDs.contains(row.volume_id)
                    && self.volumes.contains(where: { $0.id == row.volume_id })
                }.map { ($0.volume_id, $0.shelf_label) })
            for collision in result?.collisions ?? [] {
                let members = collision.volume_ids.filter { catalogIDs.contains($0) }
                if members.count > 1 {
                    for disk in members { self.shelfTagConflicts[disk] = members }
                }
            }
        }
    }

    func saveShelfLabel(on volume: Int64, text: String) {
        guard volumes.contains(where: { $0.id == volume }) else {
            status = "Selected catalog disk changed; private shelf tag not saved."
            return
        }
        let label = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard label.count <= 80 && !label.unicodeScalars.contains(where: {
            CharacterSet.controlCharacters.contains($0)
        }) else {
            status = "Shelf tag must be one line, at most 80 characters."
            return
        }
        let scope = physicalScope
        // Reject an older read that might otherwise overwrite the newly saved tag.
        shelfLabelRequestGeneration += 1
        var args = physicalArguments([], probeOK: false)
        args["op"] = "physical_shelf_label_set"
        args["volume_id"] = volume
        args["shelf_label"] = label
        args["confirmed"] = true
        request(args, as: ShelfLabelSaved.self) { [weak self] saved in
            guard let self, self.physicalScope == scope else { return }
            if let saved, saved.saved, saved.volume_id == volume {
                if saved.shelf_label.isEmpty { self.shelfLabels.removeValue(forKey: volume) }
                else { self.shelfLabels[volume] = saved.shelf_label }
                self.refreshShelfLabels()
                self.status = "Tag saved; shared sticker warnings require manual physical label checks."
            } else { self.status = "Private shelf tag not saved." }
        }
    }

    func physicalState(_ id: Int64) -> String {
        let now = ProcessInfo.processInfo.systemUptime
        guard physicalProbeOK,
              let verifiedAt = physicalVerifiedUptime,
              now >= verifiedAt, now - verifiedAt <= physicalStateTTL else {
            return "unverified"
        }
        switch physicalStates[id] {
        case "connected":
            // The connected state is shared by ALL native panels. Neither
            // a service label alone nor an unobserved/duplicated filesystem
            // UUID establishes current physical attachment.
            guard let verifiedID = verifiedPhysicalIDs[id],
                  !verifiedID.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
                  mountedPhysicalVolumes.filter({ $0.id == verifiedID }).count == 1 else {
                return "unverified"
            }
            return "connected"
        case "offline": return "offline"
        case "conflict": return "conflict"
        default: return "unverified"
        }
    }
    func candidateCopyOSCaption(_ id: Int64) -> String {
        switch physicalState(id) {
        case "connected": return "OS verified now"
        case "offline": return "paired, offline"
        case "conflict": return "identity conflict"
        default: return "not verified"
        }
    }
    // Physical retrieval first: exact catalog disk ID and private shelf/sticker
    // tag, then historical catalog label, metadata count and independent OS state.
    func candidateCopyDiskCaption(_ group: CandidateDiskGroup) -> String {
        let tag = shelfLabel(group.volume_id)
        let sticker = tag.isEmpty ? "no sticker tag" : "sticker: \(tag)"
        let warning = shelfConflictCaption(group.volume_id) == nil ? "" : " • SHARED STICKER"
        return "#\(group.volume_id) • \(sticker) • \(group.label) • \(group.count) metadata candidates • \(candidateCopyOSCaption(group.volume_id))\(warning)"
    }
    func candidateCopyDiskAccessibility(_ group: CandidateDiskGroup) -> String {
        let tag = shelfLabel(group.volume_id)
        return "Show \(group.count) name and size metadata candidates on catalog disk number \(group.volume_id), \(group.label). Owner sticker or shelf: \(tag.isEmpty ? "not assigned" : tag). \(candidateCopyOSCaption(group.volume_id)). \(shelfConflictCaption(group.volume_id) ?? ""). Content identity not verified."
    }
    // Sort copies by independently checked OS pairing state. The private
    // shelf label is a retrieval hint, never evidence of mounted file bytes.
    var orderedCopyDiskGroups: [CandidateDiskGroup] {
        func rank(_ disk: Int64) -> Int {
            switch physicalState(disk) {
            case "connected": return 0
            case "offline": return 1
            case "conflict": return 3
            default: return 2
            }
        }
        return copyDiskGroups.sorted {
            let left = rank($0.volume_id)
            let right = rank($1.volume_id)
            if left != right { return left < right }
            if $0.count != $1.count { return $0.count > $1.count }
            return $0.volume_id < $1.volume_id
        }
    }
    private enum CandidateClipboardAction { case pickupList, pagePaths }
    private var candidateClipboardRequestSerial = 0
    private var selectedMetadataClipboardSerial = 0

    // A loaded page is not authorization to copy historical linked CTF data
    // later. Verify the original SHA on EVERY clipboard action, then reject a
    // late reply if search source, page, focus, or latest action changed.
    private func verifyCandidateClipboardThen(_ action: CandidateClipboardAction) {
        guard pageReady, !awaitingPage, let source = copySourceID else {
            status = "Current metadata candidate page is not ready; clipboard unchanged."
            return
        }
        candidateClipboardRequestSerial += 1
        let serial = candidateClipboardRequestSerial
        let token = generation
        let scope = physicalScope
        let page = offset
        let focus = copyFocusVolumeID
        let count = entries.count
        if linkedCTFPath != nil {
            guard var pinned = linkedFields() else {
                status = "Linked CTF generation unavailable; clipboard unchanged."
                return
            }
            pinned["op"] = "ctf_volumes_pinned"
            status = "Rechecking original CTF generation before local clipboard copy…"
            request(pinned, as: LinkedCTFResult<[Volume]>.self, generationToken: token) { [weak self] reply in
                guard let self else { return }
                guard self.candidateClipboardRequestSerial == serial,
                      self.physicalScope == scope,
                      self.generation == token,
                      self.pageReady, !self.awaitingPage,
                      self.copySourceID == source,
                      self.copyFocusVolumeID == focus,
                      self.offset == page, self.entries.count == count,
                      reply?.generation == self.linkedGeneration else {
                    self.status = "Linked CTF or candidate page changed; clipboard NOT updated."
                    return
                }
                self.finishCandidateClipboard(action)
            }
        } else {
            finishCandidateClipboard(action)
        }
    }
    private func finishCandidateClipboard(_ action: CandidateClipboardAction) {
        switch action {
        case .pickupList: writePickupPlan()
        case .pagePaths: writeCandidatePagePaths()
        }
    }

    // ALL registered catalog disks, regardless of the loaded results page,
    // temporary register filter or source browse scope. Linked mode rechecks
    // the original CTF generation independently at clipboard time.
    func copyFullDiskRegister() {
        let token = generation
        let scope = physicalScope
        if linkedCTFPath != nil {
            guard var pinned = linkedFields() else {
                status = "Linked CTF generation missing; FULL disk register NOT copied."
                return
            }
            pinned["op"] = "ctf_volumes_pinned"
            status = "Rechecking original CTF SHA before copying ALL catalog disks…"
            request(pinned, as: LinkedCTFResult<[Volume]>.self, generationToken: token) { [weak self] reply in
                guard let self else { return }
                guard self.generation == token, self.physicalScope == scope,
                      reply?.generation == self.linkedGeneration,
                      let verified = reply?.result,
                      Set(verified.map(\.id)) == Set(self.volumes.map(\.id)) else {
                    self.status = "Linked source or catalog register changed; FULL disk register NOT copied."
                    return
                }
                self.writeFullDiskRegister(verified)
            }
        } else {
            writeFullDiskRegister(volumes)
        }
    }
    private func writeFullDiskRegister(_ snapshot: [Volume]) {
        let ordered = snapshot.sorted { $0.id < $1.id }
        guard !ordered.isEmpty, ordered.allSatisfy({ $0.id > 0 }),
              Set(ordered.map(\.id)).count == ordered.count else {
            status = "Catalog disk IDs missing or duplicated; FULL disk register NOT copied."
            return
        }
        let clean: (String) -> String = {
            $0.replacingOccurrences(of: "\r", with: " ")
              .replacingOccurrences(of: "\n", with: " ")
              .replacingOccurrences(of: "\t", with: " ")
        }
        var lines = [
            "WhereIsIt AI — FULL offline physical disk register",
            "\(ordered.count) catalog disk IDs; ALL REGISTERED DISKS, not a search page or current filter.",
            "Physical state is a recent, independently paired OS filesystem UUID check only. NOT proof of file presence."
        ]
        for disk in ordered {
            let sticker = shelfLabel(disk.id)
            let tag = sticker.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                ? "not assigned" : clean(sticker)
            let count = disk.entry_count.map { "\($0) recorded entries" } ?? "entry count unavailable"
            let warning = shelfConflictCaption(disk.id).map { " • WARNING: \(clean($0))" } ?? ""
            lines.append("#\(disk.id) • \(clean(disk.label)) • sticker/shelf: \(tag) • \(count) • \(physicalCaption(disk.id))\(warning)")
        }
        lines.append("Use exact catalog IDs and your physical stickers. Historical letters and catalog paths are NOT current OS mount locations.")
        lines.append("Filesystem UUIDs may be cloned. No file hashes or bytes were compared; never auto-delete metadata matches.")
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(lines.joined(separator: "\n"), forType: .string)
        status = "FULL offline disk register copied locally (all catalog IDs, regardless of filter). OS states may expire; file bytes unverified."
    }

    func copyPickupPlan() {
        verifyCandidateClipboardThen(.pickupList)
    }
    private func writePickupPlan() {
        guard pageReady, copySourceID != nil, let file = copySourceName,
              !copyDiskGroups.isEmpty else {
            status = "Find copies and wait for catalogued candidate disks first."
            return
        }
        let clean: (String) -> String = {
            $0.replacingOccurrences(of: "\r", with: " ").replacingOccurrences(of: "\n", with: " ")
        }
        let total = copyDiskGroups.reduce(0) { $0 + $1.count }
        let header = [
            "WhereIsIt AI — offline USB pickup list",
            "File (recorded catalog name): \(clean(file))",
            "\(total) name+size metadata candidates on \(copyDiskGroups.count) OTHER catalog disks; source catalog disk excluded.",
            "Order: last OS pairing check, then candidate count; exact disk IDs are authoritative.",
        ]
        let rows = orderedCopyDiskGroups.enumerated().map { index, group in
            "\(index + 1). \(clean(candidateCopyDiskCaption(group)))"
        }
        let caution = [
            "Last OS probe may be stale; recheck the actual physical disk before connecting.",
            "Sticker/shelf is an owner note, NOT USB identity. Recorded paths are historical.",
            "Matching name+size never verifies file bytes or hashes; do NOT auto-delete.",
        ]
        let text = (header + rows + caution).joined(separator: "\n")
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        status = "Disk pickup checklist copied locally. OS badges are last observations; file bytes unverified."
    }
    // Disk-only physical pickup card: the operator may have chosen a disk in
    // the left register without selecting any file. Never use historical
    // mount letters or claim a file is present from an OS filesystem UUID.
    func copySelectedDiskPickupCard() {
        guard let diskID = selectedDiskForPairing,
              let disk = volumes.first(where: { $0.id == diskID }) else {
            status = "Select a current catalog disk or file before copying its pickup card."
            return
        }
        let scope = physicalScope
        let token = generation
        if linkedMode {
            guard let fields = linkedFields() else {
                status = "Linked CTF generation missing; physical disk card NOT copied."
                return
            }
            var pinned = fields
            pinned["op"] = "ctf_volumes_pinned"
            status = "Verifying linked CTF generation before copying physical disk card…"
            request(pinned, as: LinkedCTFResult<[Volume]>.self, generationToken: token) { [weak self] reply in
                guard let self else { return }
                guard self.physicalScope == scope, self.generation == token,
                      self.selectedDiskForPairing == diskID,
                      reply?.generation == self.linkedGeneration,
                      let verifiedDisk = reply?.result.first(where: { $0.id == diskID }) else {
                    self.status = "Linked CTF generation or selected disk changed; pickup card NOT copied."
                    return
                }
                self.writeDiskPickupCard(verifiedDisk)
            }
        } else {
            writeDiskPickupCard(disk)
        }
    }
    // Do not reuse a loaded linked CTF page for local clipboard actions:
    // independently revalidate its original generation and current selection.
    func copySelectedMetadata(_ text: String, entryID: Int64, diskID: Int64) {
        selectedMetadataClipboardSerial += 1
        let serial = selectedMetadataClipboardSerial
        guard pageReady, !awaitingPage, let row = selected,
              row.id == entryID, row.volume_id == diskID else {
            status = "Current file selection is stale; clipboard NOT updated."
            return
        }
        let token = generation
        let scope = physicalScope
        if linkedMode {
            guard var pinned = linkedFields() else {
                status = "Linked CTF generation missing; clipboard NOT updated."
                return
            }
            pinned["op"] = "ctf_volumes_pinned"
            status = "Rechecking linked CTF generation before copying selected metadata…"
            request(pinned, as: LinkedCTFResult<[Volume]>.self, generationToken: token) { [weak self] reply in
                guard let self else { return }
                guard self.selectedMetadataClipboardSerial == serial,
                      self.generation == token, self.physicalScope == scope,
                      self.pageReady, !self.awaitingPage,
                      self.selected?.id == entryID, self.selected?.volume_id == diskID,
                      reply?.generation == self.linkedGeneration,
                      reply?.result.contains(where: { $0.id == diskID }) == true else {
                    self.status = "Selected file or linked source changed; clipboard NOT updated."
                    return
                }
                self.writeSelectedMetadata(text)
            }
        } else {
            writeSelectedMetadata(text)
        }
    }
    private func writeSelectedMetadata(_ text: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        status = "Current catalog metadata copied locally; physical file bytes NOT verified."
    }

    // Only called after exact source-generation revalidation in linked CTF
    // mode. The OS badge itself also expires without a fresh native probe.
    private func writeDiskPickupCard(_ disk: Volume) {
        let diskID = disk.id
        let clean: (String) -> String = {
            $0.replacingOccurrences(of: "\r", with: " ")
                .replacingOccurrences(of: "\n", with: " ")
                .replacingOccurrences(of: "\t", with: " ")
        }
        let sticker = shelfLabel(diskID)
        let count = disk.entry_count.map { "\($0) historical catalogue entries" } ?? "not reported"
        var lines = [
            "WhereIsIt AI — physical disk pickup card",
            "Catalog disk ID: #\(diskID)",
            "Recorded label: \(clean(disk.label))",
            "Owner sticker/shelf: \(sticker.isEmpty ? "not assigned" : clean(sticker))",
            "Recorded entries: \(count)",
            "Physical: \(physicalCaption(diskID))",
        ]
        if let warning = shelfConflictCaption(diskID) {
            lines.append("WARNING: \(clean(warning))")
        }
        lines += [
            "Identify the actual disk by its physical sticker; catalog labels and historical paths are NOT live OS identity.",
            "An OS filesystem UUID can be cloned. Mount state is a time-limited observation, not proof that files exist.",
            "No file bytes or hashes were compared; never auto-delete metadata matches."
        ]
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(lines.joined(separator: "\n"), forType: .string)
        status = "Exact catalog disk pickup card copied locally; neither actual file bytes nor a hardware serial were verified."
    }

    // No disk I/O: this is a clipboard manifest from exactly one completed
    // generation-pinned candidate page, NOT an export of every catalog copy.
    func copyCandidatePagePaths() {
        verifyCandidateClipboardThen(.pagePaths)
    }
    private func writeCandidatePagePaths() {
        guard pageReady, copySourceID != nil, let file = copySourceName else {
            status = "Candidate page is loading or source verification failed; paths not copied."
            return
        }
        let clean: (String) -> String = {
            $0.replacingOccurrences(of: "\r", with: " ")
                .replacingOccurrences(of: "\n", with: " ")
                .replacingOccurrences(of: "\t", with: " ")
        }
        let scope = copyFocusVolumeID.map { "Catalog disk #\($0)" } ?? "All catalogued disks"
        let first = entries.isEmpty ? 0 : offset + 1
        let last = offset + entries.count
        let header = [
            "WhereIsIt AI — recorded candidate paths, CURRENT PAGE ONLY",
            "File (recorded catalog name): \(clean(file))",
            "Scope: \(scope); loaded rows \(first)–\(last) of \(total).",
            "The clipboard contains only this loaded page (maximum 250 records), not all copies."
        ]
        let rows = entries.prefix(limit).enumerated().map { index, entry in
            let bytes = entry.size.map { "\($0) bytes" } ?? "size not recorded"
            return "\(index + 1). Disk #\(entry.volume_id) • \(clean(entry.name)) • \(clean(entry.relative_path)) • \(bytes)"
        }
        let caution = [
            "Paths are HISTORICAL catalog metadata, not current mounted file locations.",
            "Linked CTF SHA independently rechecked at clipboard action (if linked); no file bytes verified.",
            "Matching filename+size does NOT verify file contents or hashes; never auto-delete."
        ]
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString((header + rows + caution).joined(separator: "\n"), forType: .string)
        status = "Current candidate PAGE paths copied locally. Other pages, physical presence and bytes remain unverified."
    }

    var soleConnectedCandidateDisk: Int64? {
        let matches = copyDiskGroups.filter { $0.count > 0 && physicalState($0.volume_id) == "connected" }
        return matches.count == 1 ? matches.first?.volume_id : nil
    }
    func focusSoleConnectedCopy() {
        guard pageReady, let source = copySourceID, !copyDiskGroups.isEmpty else {
            status = "Find metadata copy candidates first."
            return
        }
        pendingConnectedCopyFocus = (source: source, scope: physicalScope)
        refreshPhysicalPresence()  // Fresh, complete OS probe before focus.
    }
    func physicalCaption(_ id: Int64) -> String {
        switch physicalState(id) {
        case "connected": return "OS volume connected (paired UUID verified)"
        case "offline": return "Offline: paired UUID absent in OS scan"
        case "conflict": return "OS identity conflict — resolve manually"
        default: return "Physical connection not verified"
        }
    }
    func physicalSymbol(_ id: Int64) -> String {
        switch physicalState(id) {
        case "connected": return "checkmark.circle.fill"
        case "offline": return "externaldrive.badge.xmark"
        case "conflict": return "exclamationmark.triangle.fill"
        default: return "questionmark.circle"
        }
    }
    // Only a *complete* local-volume snapshot can prove that a previously
    // paired volume is absent. Remote volumes are irrelevant to USB matching;
    // if ANY visible local mount cannot be classified or lacks a filesystem
    // UUID, return nil rather than quietly omitting it and claiming "offline".
    // Hidden local mounts count too; excluding them could produce false "offline".
    func scanPhysicalVolumes() -> [LivePhysicalVolume]? {
        let keys: Set<URLResourceKey> = [.volumeUUIDStringKey, .volumeNameKey, .volumeIsLocalKey]
        guard let urls = FileManager.default.mountedVolumeURLs(
            includingResourceValuesForKeys: Array(keys), options: []
        ) else { return nil }
        var result: [LivePhysicalVolume] = []
        for url in urls {
            guard let values = try? url.resourceValues(forKeys: keys),
                  let isLocal = values.volumeIsLocal else { return nil }
            if !isLocal { continue }
            guard let uuid = values.volumeUUIDString?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased(),
                  !uuid.isEmpty else { return nil }
            result.append(LivePhysicalVolume(
                id: "mac:" + uuid,
                name: values.volumeName ?? url.lastPathComponent,
                currentMount: url.path
            ))
        }
        return result
    }
    func physicalArguments(_ observed: [LivePhysicalVolume], probeOK: Bool) -> [String: Any] {
        [
            "user_store": physicalUserStore,
            "catalog_key": physicalScope,
            "platform": "macos",
            "volume_ids": volumes.map(\.id),
            "observed_ids": observed.map(\.id),
            "probe_ok": probeOK
        ]
    }
    // Mount notifications are not proof of USB identity. Invalidate immediately
    // so a former connected badge and pending root/candidate copy cannot survive
    // unplugging, while the next complete OS + service scan determines status.
    func invalidatePhysicalPresenceAfterMountEvent() {
        physicalMountEventGeneration += 1
        mountActionGeneration += 1
        physicalCheckGeneration += 1  // discard an older asynchronous status reply
        physicalVerifiedUptime = nil
        physicalStates = [:]
        verifiedPhysicalIDs = [:]
        physicalProbeOK = false
        mountedPhysicalVolumes = []
        selectedPhysicalID = ""
        pendingMountCopy = nil
        pendingConnectedCopyFocus = nil
    }
    func recheckPhysicalPresenceAfterMountEvent() {
        invalidatePhysicalPresenceAfterMountEvent()
        refreshPhysicalPresence()
    }
    func refreshPhysicalPresence() {
        // Start TTL before enumeration, not on a delayed async service reply.
        let scannedAt = ProcessInfo.processInfo.systemUptime
        physicalCheckGeneration += 1
        let checkToken = physicalCheckGeneration
        let scope = physicalScope
        let probe = scanPhysicalVolumes()
        physicalProbeOK = probe != nil
        mountedPhysicalVolumes = probe ?? []
        if !pairablePhysicalVolumes.contains(where: { $0.id == selectedPhysicalID }) {
            selectedPhysicalID = ""
        }
        // Do not carry a positive connection claim through a failed or pending probe.
        physicalStates = [:]
        verifiedPhysicalIDs = [:]
        physicalVerifiedUptime = nil
        var args = physicalArguments(mountedPhysicalVolumes, probeOK: physicalProbeOK)
        args["op"] = "physical_status"
        let observed = mountedPhysicalVolumes.map(\.id)
        request(args, as: PhysicalStateReply.self) { [weak self] result in
            guard let self, self.physicalCheckGeneration == checkToken,
                  self.physicalScope == scope,
                  self.mountedPhysicalVolumes.map(\.id) == observed else { return }
            // No positive status survives a missing/failed service response.
            if let result, self.physicalProbeOK == result.probe_ok,
               result.probe_ok,
               ProcessInfo.processInfo.systemUptime - scannedAt <= self.physicalStateTTL {
                self.physicalVerifiedUptime = scannedAt
                self.physicalStates = Dictionary(
                    uniqueKeysWithValues: result.items.map { ($0.volume_id, $0.state) }
                )
                self.verifiedPhysicalIDs = Dictionary(uniqueKeysWithValues:
                    result.items.compactMap { row -> (Int64, String)? in
                        guard row.state == "connected", let id = row.verified_physical_id else { return nil }
                        return (row.volume_id, id)
                    })
            }
            if let pending = self.pendingMountCopy {
                self.pendingMountCopy = nil
                guard pending.scope == scope, self.selectedDiskForPairing == pending.disk,
                      self.physicalState(pending.disk) == "connected",
                      let verifiedID = self.verifiedPhysicalIDs[pending.disk],
                      let mounted = self.mountedPhysicalVolumes.first(where: { $0.id == verifiedID }),
                      self.mountedPhysicalVolumes.filter({ $0.id == verifiedID }).count == 1,
                      let secondProbe = self.scanPhysicalVolumes(),
                      // Count the UUID itself, NOT UUID+path: a clone may
                      // appear at another mount between the two OS probes.
                      let freshMatch = secondProbe.first(where: { $0.id == verifiedID }),
                      secondProbe.filter({ $0.id == verifiedID }).count == 1,
                      freshMatch.currentMount == mounted.currentMount else {
                    self.status = "Current OS mount root NOT verified; source, pairing or USB volume changed."
                    return
                }
                if pending.openInFinder {
                    // OS root comes ONLY from the latest FileManager mounted-
                    // volume UUID observation, never a historical catalog path.
                    let url = URL(fileURLWithPath: mounted.currentMount, isDirectory: true)
                    guard mounted.currentMount.hasPrefix("/"),
                          NSWorkspace.shared.open(url) else {
                        self.status = "Finder could not open the verified OS disk root."
                        return
                    }
                    self.status = "Finder accepted the OS-verified disk root request; catalogued files and bytes UNVERIFIED."
                } else {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(mounted.currentMount, forType: .string)
                    self.status = "CURRENT OS root copied after two UUID probes; catalogued file presence and bytes unverified."
                }
            }
            if let pending = self.pendingConnectedCopyFocus {
                self.pendingConnectedCopyFocus = nil
                guard pending.source == self.copySourceID, pending.scope == scope else { return }
                if self.physicalProbeOK, result?.probe_ok == true,
                   let disk = self.soleConnectedCandidateDisk {
                    self.focusCopyCandidates(on: disk)
                } else {
                    self.status = self.physicalProbeOK && result != nil
                        ? "Zero or several candidate disks are OS-verified; select one explicitly."
                        : "OS mount scan incomplete; no physical copy disk was verified."
                }
            }
        }
    }
    // These actions operate ONLY on the presently mounted, explicitly paired
    // OS volume root, never on a recorded catalog file or historical drive path.
    func copyCurrentOSMountRoot() { performCurrentOSRootAction(openInFinder: false) }
    func openCurrentOSMountRoot() { performCurrentOSRootAction(openInFinder: true) }

    private func performCurrentOSRootAction(openInFinder: Bool) {
        guard let disk = selectedDiskForPairing, volumes.contains(where: { $0.id == disk }) else {
            status = "Select a current file or catalog disk before using its live OS root."
            return
        }
        mountActionGeneration += 1
        let actionToken = mountActionGeneration
        pendingMountCopy = nil
        let scope = physicalScope
        let mountEventToken = physicalMountEventGeneration
        // Incomplete linked session is NOT a local SQLite session: fail closed.
        if linkedCTFPath != nil {
            guard var pinned = linkedFields() else {
                status = "Linked CTF generation missing; OS root NOT opened or copied."
                return
            }
            pinned["op"] = "ctf_volumes_pinned"
            status = "Rechecking linked source generation before using OS root…"
            request(pinned, as: LinkedCTFResult<[Volume]>.self) { [weak self] reply in
                guard let self, self.physicalScope == scope,
                      self.mountActionGeneration == actionToken,
                      self.physicalMountEventGeneration == mountEventToken,
                      self.selectedDiskForPairing == disk else { return }
                guard reply != nil else {
                    self.status = "Linked CTF generation could not be verified; no OS root opened or copied."
                    return
                }
                self.pendingMountCopy = (disk, scope, openInFinder)
                self.refreshPhysicalPresence()
            }
        } else {
            pendingMountCopy = (disk, scope, openInFinder)
            refreshPhysicalPresence()
        }
    }

    func pairSelectedPhysicalDisk() {
        guard let volumeID = selectedDiskForPairing,
              let candidate = pairablePhysicalVolumes.first(where: { $0.id == selectedPhysicalID }) else {
            status = "Select a catalog disk and a freshly verified OS-mounted volume."
            physicalStates = [:]
            return
        }
        let warning = NSAlert()
        warning.alertStyle = .warning
        warning.messageText = "Pair catalog disk with this mounted OS volume?"
        warning.informativeText = """
        Catalog disk: \(volumeLabel(volumeID))
        Current OS volume: \(candidate.name) at \(candidate.currentMount)

        Check the physical USB disk label before pairing. Volume UUID is a filesystem identity, not a manufacturer hardware serial. A cloned volume can have the same UUID. No CTF or file contents will be modified.
        """
        warning.addButton(withTitle: "Pair explicitly")
        warning.addButton(withTitle: "Cancel")
        guard warning.runModal() == .alertFirstButtonReturn else { return }
        // Reprobe after the modal, not before: the disk may have been unplugged.
        guard let freshProbe = scanPhysicalVolumes(),
              // Reject a same-UUID clone at ANY other mount, even when the
              // originally selected UUID+path still matches exactly once.
              let freshMatch = freshProbe.first(where: { $0.id == candidate.id }),
              freshProbe.filter({ $0.id == candidate.id }).count == 1,
              freshMatch.currentMount == candidate.currentMount else {
            physicalStates = [:]
            status = "OS volume changed during confirmation; pairing refused. Refresh mounts."
            return
        }
        let scope = physicalScope
        var args = physicalArguments(freshProbe, probeOK: true)
        args["op"] = "physical_pair"
        args["volume_id"] = volumeID
        args["physical_id"] = candidate.id
        args["confirmed"] = true
        request(args, as: PhysicalPairReply.self) { [weak self] result in
            guard let self, self.physicalScope == scope else { return }
            self.refreshPhysicalPresence()
            if result?.paired == true {
                self.status = "Paired to live OS volume UUID. Rechecks run every six seconds."
            } else {
                self.status = "Pairing refused: another disk, stale identity or explicit unpair required."
            }
        }
    }
    func unpairSelectedPhysicalDisk() {
        guard let volumeID = selectedDiskForPairing else {
            status = "Select a catalog disk to remove its OS pairing."
            return
        }
        let warning = NSAlert()
        warning.messageText = "Remove this physical disk pairing?"
        warning.informativeText = "Only the user-owned OS identity mapping for \(volumeLabel(volumeID)) will be removed. Catalog and linked CTF remain unchanged."
        warning.addButton(withTitle: "Remove pairing")
        warning.addButton(withTitle: "Cancel")
        guard warning.runModal() == .alertFirstButtonReturn else { return }
        let scope = physicalScope
        var args = physicalArguments(mountedPhysicalVolumes, probeOK: physicalProbeOK)
        args["op"] = "physical_unpair"
        args["volume_id"] = volumeID
        args["confirmed"] = true
        request(args, as: PhysicalUnpairReply.self) { [weak self] result in
            guard let self, self.physicalScope == scope else { return }
            self.refreshPhysicalPresence()
            self.status = result != nil ? "Pairing removed; disk is NOT VERIFIED." : "Could not remove user-side pairing."
        }
    }

    func refreshVolumes() {
        refreshCategories()
        if linkedMode {
            guard let fields = linkedFields() else {
                generation += 1
                volumes = []; entries = []; selection = nil
                pageReady = false; awaitingPage = false; retryPageAvailable = true
                copyDiskGroups = []; physicalStates = [:]; verifiedPhysicalIDs = [:]
                physicalVerifiedUptime = nil
                status = "Linked CTF generation missing; no SQLite catalog fallback."
                return
            }
            let token = generation
            let scope = physicalScope
            var requestData: [String: Any] = ["op": "ctf_volumes_pinned"]
            requestData.merge(fields) { _, newValue in newValue }
            request(requestData, as: LinkedCTFResult<[Volume]>.self) { [weak self] response in
                guard let self, let response, self.generation == token,
                      self.physicalScope == scope,
                      response.generation == self.linkedGeneration else { return }
                self.volumes = response.result
                self.refreshPhysicalPresence()
                self.refreshShelfLabels()
                self.status = "\(response.result.count) linked CTF disks • read-only; search or select a disk"
            }
            return
        }
        request(["op": "volumes"], as: [Volume].self) { [weak self] values in
            guard let self, let values else { return }
            self.volumes = values
            self.refreshPhysicalPresence()
            self.refreshShelfLabels()
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
    func selectVolume() {
        copySourceID = nil; copySourceName = nil; copyReturn = nil; candidateFolderReturn = nil
        query = ""; appliedQuery = ""
        stack = [(nil, "/")]; offset = 0; showingDuplicates = false; load()
    }
    func focusCopyCandidates(on volume: Int64?) {
        guard copySourceID != nil else { return }
        copyFocusVolumeID = volume
        offset = 0
        load()
    }
    // Historical path is only a breadcrumb: parent_id and disk ID, not a
    // guessed OS mount, decide which catalogued directory to browse.
    static func recordedParentBreadcrumb(_ relativePath: String) -> String? {
        guard !relativePath.isEmpty, !relativePath.contains("\0"),
              !relativePath.hasSuffix("/"), !relativePath.hasSuffix("\\") else { return nil }
        let normalized = relativePath.replacingOccurrences(of: "\\", with: "/")
        guard let slash = normalized.lastIndex(of: "/") else { return "/" }
        let parent = String(normalized[..<slash])
        return parent.isEmpty ? "/" : parent
    }

    func browseSelectedFileFolder() {
        guard pageReady, let entry = selected,
              entry.kind == "file", volumes.contains(where: { $0.id == entry.volume_id }),
              let breadcrumb = Self.recordedParentBreadcrumb(entry.relative_path) else {
            status = "Select a current file with a recorded parent on a registered catalog disk."
            return
        }
        if let parentID = entry.parent_id, parentID <= 0 || parentID == entry.id {
            status = "Invalid catalog parent ID; recorded folder navigation refused."
            return
        }
        if let sourceID = copySourceID {
            candidateFolderReturn = CandidateFolderReturnContext(
                sourceID: sourceID, sourceName: copySourceName,
                focusVolumeID: copyFocusVolumeID, offset: offset, diskFilter: diskFilter,
                selectedFile: ReturnSelection(id: entry.id, diskID: entry.volume_id))
        } else if candidateFolderReturn == nil {
            copyReturn = CopyReturnContext(volumeID: volumeID, stack: stack,
                query: query, appliedQuery: appliedQuery, offset: offset,
                showingDuplicates: showingDuplicates, appliedDuplicateMode: appliedDuplicateMode,
                selectedFile: ReturnSelection(id: entry.id, diskID: entry.volume_id))
        }
        copySourceID = nil; copySourceName = nil
        copyFocusVolumeID = nil; copyDiskGroups = []
        if diskFilter.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() == "copies:" {
            diskFilter = ""
        }
        volumeID = entry.volume_id
        stack = [(nil, "/")]
        if let parentID = entry.parent_id {
            stack.append((parentID, breadcrumb))
        }
        query = ""; appliedQuery = ""; showingDuplicates = false; offset = 0
        load() // ctf_children_pinned/children; linked source SHA checked per page.
    }

    // Literal filename match, independent of recorded size; no byte/hash equality.
    static func exactFilenameQuery(_ name: String) -> String? {
        guard !name.isEmpty, !name.contains("\0") else { return nil }
        let escaped = name.replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
        let result = "name_exact:\"\(escaped)\""
        return result.utf8.count <= 4096 ? result : nil
    }
    func searchSameFilenameAcrossDisks() {
        guard pageReady, copySourceID == nil, let entry = selected,
              entry.kind == "file",
              let expression = Self.exactFilenameQuery(entry.name) else {
            status = "Select a current file outside copy mode; filename must fit 4096 bytes."
            return
        }
        candidateFolderReturn = nil
        copyReturn = CopyReturnContext(volumeID: volumeID, stack: stack,
            query: query, appliedQuery: appliedQuery, offset: offset,
            showingDuplicates: showingDuplicates, appliedDuplicateMode: appliedDuplicateMode,
            selectedFile: ReturnSelection(id: entry.id, diskID: entry.volume_id))
        copySourceID = nil; copySourceName = nil
        volumeID = nil; stack = [(nil, "/")]; offset = 0
        showingDuplicates = false; query = expression; appliedQuery = expression
        load() // existing pinned CTF search, with generation check on every page.
    }
    // Size-only search exposes possible renamed files; name, file bytes and
    // media identity are NOT compared. Limit the key to valid signed bytes.
    static func exactRecordedSizeQuery(_ size: Int64?) -> String? {
        guard let size, size > 0 else { return nil }
        return "size:\(size) AND kind:file"
    }
    func searchSameRecordedSizeAcrossDisks() {
        guard pageReady, copySourceID == nil, let entry = selected,
              entry.kind == "file",
              let expression = Self.exactRecordedSizeQuery(entry.size) else {
            status = "Select a current file with a positive recorded byte size outside copy mode."
            return
        }
        candidateFolderReturn = nil
        copyReturn = CopyReturnContext(volumeID: volumeID, stack: stack,
            query: query, appliedQuery: appliedQuery, offset: offset,
            showingDuplicates: showingDuplicates, appliedDuplicateMode: appliedDuplicateMode,
            selectedFile: ReturnSelection(id: entry.id, diskID: entry.volume_id))
        copySourceID = nil; copySourceName = nil
        volumeID = nil; stack = [(nil, "/")]; offset = 0
        showingDuplicates = false; query = expression; appliedQuery = expression
        load() // source SHA is freshly checked by the existing pinned CTF search.
    }
    func copiesOfSelected() {
        guard let entry = selected, entry.kind == "file", entry.size != nil else {
            status = "Select a file with recorded size to find catalogued metadata copies."
            return
        }
        candidateFolderReturn = nil
        copyReturn = CopyReturnContext(volumeID: volumeID, stack: stack,
            query: query, appliedQuery: appliedQuery, offset: offset,
            showingDuplicates: showingDuplicates, appliedDuplicateMode: appliedDuplicateMode,
            selectedFile: ReturnSelection(id: entry.id, diskID: entry.volume_id))
        copySourceID = entry.id; copySourceName = entry.name
        copyFocusVolumeID = nil; copyDiskGroups = []
        volumeID = nil; stack = [(nil, "/")]; offset = 0
        showingDuplicates = false; query = ""; appliedQuery = ""
        load()
    }
    // Distinguish browsing the catalogued disk from searching within a scope.
    func browseDisk(_ id: Int64) {
        copySourceID = nil; copySourceName = nil; copyReturn = nil; candidateFolderReturn = nil
        volumeID = id
        query = ""; appliedQuery = ""
        stack = [(nil, "/")]; offset = 0
        showingDuplicates = false
        load()
    }
    func searchAcrossAllDisks() { search() }

    func searchInSelectedDisk() {
        // Explicit scope selection, including invalid attempts, cancels typeahead.
        cancelPendingLiveFilenameSearch()
        guard volumeID != nil else {
            status = "Choose a catalog disk for an explicit disk-only search."
            return
        }
        copySourceID = nil; copySourceName = nil; copyReturn = nil; candidateFolderReturn = nil
        appliedQuery = query.trimmingCharacters(in: .whitespacesAndNewlines)
        showingDuplicates = false; offset = 0; load()
    }

    var resultScopeCaption: String {
        if copySourceID != nil { return "COPY CANDIDATES • OTHER CATALOG DISKS ONLY" }
        if showingDuplicates { return "METADATA DUPLICATES • ALL CATALOGUED DISKS" }
        if !appliedQuery.isEmpty {
            if volumeID == nil && appliedQuery.hasPrefix("size:")
                && appliedQuery.hasSuffix(" AND kind:file") {
                return "EXACT RECORDED SIZE • ALL DISKS • FILE CONTENT UNVERIFIED"
            }
            return volumeID.map { "SEARCH IN CATALOG DISK: " + volumeLabel($0) }
                ?? "SEARCH • ALL CATALOGUED DISKS"
        }
        return volumeID.map { "BROWSE CATALOG DISK: " + volumeLabel($0) + " • " + currentPath }
            ?? "SEARCH ACROSS ALL DISKS"
    }
    private func loadRestoring(_ row: ReturnSelection) {
        pendingReturnSelection = (token: generation + 1, row: row)
        load() // Always a fresh bounded and generation-pinned service page.
    }
    func load() {
        // Every explicit navigation/page request invalidates pending typed intent
        // before an asynchronous response can turn a scoped view into global search.
        cancelPendingLiveFilenameSearch()
        generation += 1; let token = generation
        pageRequestStartedUptime = ProcessInfo.processInfo.systemUptime
        if pendingReturnSelection?.token != token { pendingReturnSelection = nil }
        if pendingPageFocusToken != token { pendingPageFocusToken = nil }
        awaitingPage = true; retryPageAvailable = false
        // Clear previous-scope rows immediately; an old page must not be actionable.
        selection = nil; entries = []; total = 0; hasMore = false; pageReady = false
        if linkedMode && linkedFields() == nil {
            awaitingPage = false
            pendingPageFocusToken = nil
            retryPageAvailable = true
            copyDiskGroups = []
            status = "Linked CTF generation missing; SQLite search fallback refused."
            return
        }
        var args: [String: Any] = ["offset": offset, "limit": limit]
        if let volumeID { args["volume_id"] = volumeID }
        if let copySourceID {
            args["entry_id"] = copySourceID
            if let copyFocusVolumeID { args["candidate_volume_id"] = copyFocusVolumeID }
            args["other_disks_only"] = true
            status = "Finding name+size metadata candidates on OTHER catalog disks…"
            if let fields = linkedFields() {
                args["op"] = "ctf_copy_candidates_pinned"
                args.merge(fields) { _, newValue in newValue }
                request(args, as: LinkedCTFResult<EntryPage>.self, generationToken: token) { [weak self] reply in
                    self?.acceptPage(reply?.result, token: token)
                }
            } else {
                args["op"] = "copy_candidates"
                request(args, as: EntryPage.self, generationToken: token) { [weak self] page in
                    self?.acceptPage(page, token: token)
                }
            }
            return
        }
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
                awaitingPage = false
                status = "Select a linked CTF disk or search all disks."; return
            }
            args.merge(fields) { _, newValue in newValue }
            status = showingDuplicates
                ? "Finding read-only CTF metadata duplicates (may be slow on huge catalogs)…"
                : "Searching verified linked CTF…"
            request(args, as: LinkedCTFResult<EntryPage>.self, generationToken: token) { [weak self] answer in
                self?.acceptPage(answer?.result, token: token)
            }
            return
        }
        if showingDuplicates {
            args["op"] = "duplicates"; args["mode"] = appliedDuplicateMode; args["query"] = appliedQuery
        } else if !appliedQuery.isEmpty {
            args["op"] = "search_fast"; args["query"] = appliedQuery
        } else if volumeID != nil {
            args["op"] = "children"; args["parent_id"] = stack.last.flatMap { $0.0 }.map { $0 as Any } ?? NSNull()
        } else { entries = []; total = 0; hasMore = false; awaitingPage = false; status = "Choose a disk or enter an expression to search all disks."; return }
        status = showingDuplicates ? "Finding duplicate candidates by metadata…" : "Loading…"
        request(args, as: EntryPage.self, generationToken: token) { [weak self] page in
            self?.acceptPage(page, token: token)
        }
    }
    // Stop-waiting is presentation cancellation, never a CTF or SQLite write.
    // The core reader may finish; the page generation gates every callback.
    func stopWaitingForPage() {
        guard awaitingPage else { return }
        generation += 1
        pendingReturnSelection = nil
        pendingPageFocusToken = nil
        pageRequestStartedUptime = nil
        awaitingPage = false
        retryPageAvailable = true
        pageReady = false
        selection = nil; entries = []; copyDiskGroups = []
        total = 0; hasMore = false
        status = "Stopped waiting; old results discarded. Background read may finish. Retry rechecks source SHA."
    }
    func retryCurrentPage() {
        guard retryPageAvailable else { return }
        load() // Keeps the explicit search/copy scope, starts a fresh pinned read.
    }
    func acceptPage(_ page: EntryPage?, token: Int) {
        guard generation == token else { return }
        // Wall-time from a native page request through FFI, SHA verification
        // and the response; not a synthetic benchmark or accepted p95.
        let elapsedMs = pageRequestStartedUptime.map {
            max(0, Int((ProcessInfo.processInfo.systemUptime - $0) * 1000))
        }
        pageRequestStartedUptime = nil
        awaitingPage = false
        guard let page else {
            pendingReturnSelection = nil
            pendingPageFocusToken = nil
            retryPageAvailable = true // Includes pinned CTF SHA changes or read errors.
            return
        }
        applyPage(page, token: token)
        if let elapsedMs {
            recordGlobalSearchLatency(elapsedMs)
            status += " • local page \(elapsedMs) ms (not p95)"
        }
    }
    func applyPage(_ page: EntryPage, token: Int) {
        guard generation == token else { return }
        pageReady = true
        awaitingPage = false; retryPageAvailable = false
        entries = page.items; total = page.total ?? (offset + page.items.count)
        if let pending = pendingReturnSelection, pending.token == token {
            pendingReturnSelection = nil
            let matches = page.items.filter {
                $0.id == pending.row.id && $0.volume_id == pending.row.diskID
            }
            if matches.count == 1 {
                selection = pending.row.id
                restoredSelectionNonce += 1
            } else {
                status = "Back: original entry absent or ambiguous; no stale row selected."
            }
        }
        // Only the newly verified page may select its first row. A duplicate
        // identifier or an empty page leaves selection/focus unassigned.
        if pendingPageFocusToken == token {
            pendingPageFocusToken = nil
            if let first = page.items.first,
               page.items.filter({ $0.id == first.id }).count == 1 {
                selection = first.id
                restoredSelectionNonce += 1
            }
        }
        copyDiskGroups = copySourceID == nil ? [] : (page.disk_groups ?? [])
        hasMore = page.has_more ?? (offset + page.items.count < total)
        status = "\(page.items.isEmpty ? 0 : offset + 1)–\(offset + page.items.count)"
            + (page.total.map { " of \($0) items" } ?? (hasMore ? " items • more available" : " items"))
            + (copySourceID != nil
                ? " • \(page.distinct_disks ?? 0) OTHER catalog disks"
                    + (copyFocusVolumeID == nil ? "" : " • focused from \(page.all_total ?? 0) all-disk candidates")
                    + " • exact name+size metadata, NOT content verified"
                : (showingDuplicates ? " • metadata candidates; content not verified" : ""))
    }
    func search() {
        copySourceID = nil; copySourceName = nil; copyReturn = nil; candidateFolderReturn = nil
        // The primary Search/Enter action is global, even after browsing a disk.
        volumeID = nil; stack = [(nil, "/")]
        appliedQuery = query.trimmingCharacters(in: .whitespacesAndNewlines)
        showingDuplicates = false; offset = 0; load()
    }
    func duplicates() {
        copySourceID = nil; copySourceName = nil; copyReturn = nil; candidateFolderReturn = nil
        volumeID = nil; stack = [(nil, "/")]
        appliedQuery = query.trimmingCharacters(in: .whitespacesAndNewlines)
        appliedDuplicateMode = duplicateMode; showingDuplicates = true; offset = 0; load()
    }
    func back() {
        if let candidate = candidateFolderReturn {
            // A fresh pinned candidate query, never replay an old result page.
            candidateFolderReturn = nil
            copySourceID = candidate.sourceID
            copySourceName = candidate.sourceName
            copyFocusVolumeID = candidate.focusVolumeID
            copyDiskGroups = []
            volumeID = nil; stack = [(nil, "/")]
            query = ""; appliedQuery = ""; showingDuplicates = false
            offset = candidate.offset; diskFilter = candidate.diskFilter
            loadRestoring(candidate.selectedFile); return
        }
        if let context = copyReturn {
            copySourceID = nil; copySourceName = nil; copyReturn = nil; candidateFolderReturn = nil
            volumeID = context.volumeID; stack = context.stack
            query = context.query; appliedQuery = context.appliedQuery
            offset = context.offset; showingDuplicates = context.showingDuplicates
            appliedDuplicateMode = context.appliedDuplicateMode
            loadRestoring(context.selectedFile); return
        }
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
            self.globalSearchLatencyMs.removeAll()
            self.physicalStates = [:]; self.verifiedPhysicalIDs = [:]; self.pendingMountCopy = nil; self.mountActionGeneration += 1
            self.physicalVerifiedUptime = nil
            self.shelfLabels = [:]; self.selectedPhysicalID = ""
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
        verifiedOverlay = nil
        verifiedOverlayOffset = 0
        verifiedOverlayLoading = false
        showingVerifiedOverlay = false
        showingAnnotationReview = false
    }
    func reviewAnnotations() {
        guard linkedMode, mutationAllowed else { return }
        let linkedAtOpen = linkedGeneration
        let panel = NSOpenPanel()
        panel.title = "Choose your existing .wia.sqlite notes file (review and explicit actions)"
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
    func loadVerifiedOverlayPage(offset: Int) {
        guard showingAnnotationReview, linkedMode, mutationAllowed,
              let fields = linkedFields(), let store = annotationReviewStore,
              offset >= 0, !annotationConfirmationBusy, !annotationReviewLoading,
              !verifiedOverlayLoading else { return }
        if let prior = verifiedOverlay, prior.isDisplaySafe(expectedOffset: verifiedOverlayOffset),
           offset > 0 && Int64(offset) >= prior.total { return }
        let session = annotationReviewSession
        let token = generation
        verifiedOverlayOffset = offset
        verifiedOverlay = nil
        verifiedOverlayLoading = true
        showingVerifiedOverlay = true
        annotationConsent = nil
        annotationSelectedOldID = nil
        annotationSelectedCandidateID = nil
        var args = fields
        args["op"] = "ctf_annotations_activated_list_pinned"
        args["annotation_db"] = store
        args["offset"] = offset
        args["limit"] = annotationPageSize
        status = "Independently verifying active notes and stale audit counts…"
        request(args, as: LinkedCTFResult<VerifiedOverlayPage>.self,
                generationToken: token) { [weak self] response in
            guard let self, self.annotationReviewSession == session,
                  self.annotationReviewStore == store, self.verifiedOverlayOffset == offset,
                  self.showingAnnotationReview, self.showingVerifiedOverlay else { return }
            self.verifiedOverlayLoading = false
            if let page = response?.result, page.isDisplaySafe(expectedOffset: offset) {
                self.verifiedOverlay = page
                self.status = "\(page.total) verified active notes; \(page.conflicts) stale/conflicting audits excluded."
            } else {
                self.verifiedOverlay = nil
                self.status = "Overlay read failed validation; no private notes displayed."
            }
        }
    }
    func backToAnnotationConflicts() {
        guard showingAnnotationReview, !verifiedOverlayLoading else { return }
        showingVerifiedOverlay = false
        verifiedOverlay = nil
        // The old review page could now be stale: reload it through pinned SHA.
        loadAnnotationReviewPage(offset: annotationReviewOffset)
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
        guard !linkedMode || ["csv", "parquet", "json"].contains(exportFormat) else {
            status = "Linked CTF supports CSV/Parquet/JSON; the original is read-only."
            return
        }
        let chosenFormat = exportFormat
        let selectedMode = showingDuplicates ? "duplicates" : (appliedQuery.isEmpty && volumeID != nil ? "children" : "search")
        let selectedQuery = appliedQuery
        let selectedDuplicateMode = appliedDuplicateMode
        let selectedVolume = volumeID
        let selectedParent = stack.last.flatMap { $0.0 }
        let originalDatabase = database
        let originalLinkedPath = linkedCTFPath
        let originalGeneration = linkedGeneration
        let panel = NSSavePanel()
        panel.title = chosenFormat == "ctf" ? "Export entire catalog as CTF" : (selectedMode == "duplicates" ? "Export all matching duplicate candidates" : "Export current branch or query results")
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
                args["mode"] = selectedMode
                args["query"] = selectedQuery
                args["duplicate_mode"] = selectedDuplicateMode
                if let selectedVolume { args["volume_id"] = selectedVolume }
                if selectedMode != "duplicates" {
                    args["parent_id"] = selectedParent.map { $0 as Any } ?? NSNull()
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


// The catalog is authoritative for the *recorded* disk and path, not for
// current USB mount status. Never infer "connected" from a historical path.
@MainActor struct DiskLocatorInspector: View {
    @ObservedObject var model: Catalog
    @State private var shelfDraft = ""
    @State private var shelfDraftDiskID: Int64?

    // Selected-file copy actions are handled by the observable model, which
    // checks live selection and original CTF generation before clipboard use.
    // A portable shopping-list-style retrieval note, never a live file path.
    // Physical state is the most recent explicit OS probe, not file existence.
    // Kept separate from the large file inspector to bound SwiftUI type-checking
    // and allow tagging a selected register disk even without a file result.
    @ViewBuilder private var shelfTagEditor: some View {
        if let diskID = model.selectedDiskForPairing {
            Text("OWNER STICKER / SHELF TAG (manual, not OS identity)")
                .font(.caption.bold()).foregroundStyle(.secondary)
            HStack(spacing: 4) {
                TextField("Sticker number / shelf", text: $shelfDraft)
                    .accessibilityLabel("Private sticker or shelf tag for selected catalog disk")
                    .help("Your retrieval hint, never evidence of a mounted USB disk")
                Button("Save") {
                    guard shelfDraftDiskID == diskID else { return }
                    model.saveShelfLabel(on: diskID, text: shelfDraft)
                }
                .accessibilityLabel("Save private shelf tag for selected catalog disk")
            }.font(.caption)
            if let warning = model.shelfConflictCaption(diskID) {
                Label(warning, systemImage: "exclamationmark.triangle.fill")
                    .font(.caption).foregroundStyle(.orange)
                    .accessibilityLabel(warning)
            }
            Text("Saved to your private user database, scoped to this catalog generation.")
                .font(.caption2).foregroundStyle(.secondary)
            Button("Copy disk pickup card") { model.copySelectedDiskPickupCard() }
                .accessibilityLabel("Copy physical disk pickup card using the exact catalog ID and optional private sticker")
                .help("Works without a file selection. OS UUID state can expire; no live file proof.")
            Divider()
        }
    }

    @ViewBuilder private var selectedFileDetails: some View {
                if let entry = model.selected {
                    Label(model.volumeLabel(entry.volume_id), systemImage: "externaldrive")
                        .font(.headline)
                        .textSelection(.enabled)
                    if let warning = model.shelfConflictCaption(entry.volume_id) {
                         Label(warning, systemImage: "exclamationmark.triangle.fill")
                             .font(.caption).foregroundStyle(.orange)
                             .accessibilityLabel(warning)
                     }
                     Text("Catalog disk ID: \(entry.volume_id)")
                        .font(.caption).foregroundStyle(.secondary)
                    Label(model.physicalCaption(entry.volume_id), systemImage: model.physicalSymbol(entry.volume_id))
                        .foregroundStyle(model.physicalState(entry.volume_id) == "connected" ? Color.green : Color.orange)
                        .font(.callout)
                    if model.linkedMode {
                        Label("Linked CTF — read-only", systemImage: "lock")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                    Divider()
                    Text("FILE").font(.caption.bold()).foregroundStyle(.secondary)
                    Text(entry.name).font(.subheadline.weight(.semibold))
                        .textSelection(.enabled)
                    Text("RECORDED PATH").font(.caption.bold()).foregroundStyle(.secondary)
                    Text(entry.relative_path)
                        .font(.callout.monospaced())
                        .textSelection(.enabled)
                        .fixedSize(horizontal: false, vertical: true)
                    if let bytes = entry.size { Text("Size: \(ByteCountFormatter.string(fromByteCount: bytes, countStyle: .file))") }
                    if let description = entry.description, !description.isEmpty {
                        Text("DESCRIPTION").font(.caption.bold()).foregroundStyle(.secondary)
                        Text(description).font(.caption)
                            .textSelection(.enabled)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    if let count = entry.duplicate_count {
                        Text("Metadata candidates: \(count) (not verified by content)")
                            .font(.caption).foregroundStyle(.secondary)
                    } else {
                        Text("Duplicate count not calculated")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                    Button("Copy disk label") {
                        model.copySelectedMetadata(model.volumeLabel(entry.volume_id), entryID: entry.id, diskID: entry.volume_id)
                    }
                    Button("Copy recorded path") {
                        model.copySelectedMetadata(entry.relative_path, entryID: entry.id, diskID: entry.volume_id)
                    }
                    Button("Copy retrieval note") {
                        model.copySelectedMetadata(model.retrievalNote(for: entry), entryID: entry.id, diskID: entry.volume_id)
                    }
                        .keyboardShortcut("c", modifiers: [.command, .shift])
                        .accessibilityLabel("Copy catalog disk, shelf and recorded file details")
                        .help("Copy only catalog metadata and last OS probe state; no file-content verification")
                    Button("Show disk in register") { model.revealSelectedDiskInRegister() }
                        .keyboardShortcut("j", modifiers: [.command])
                        .accessibilityLabel("Find selected file's exact catalog disk in the left register")
                        .help("Filter register by exact catalog ID; no scope change or USB verification")
                    Button("Browse recorded containing folder") { model.browseSelectedFileFolder() }
                        .disabled(!model.pageReady || entry.kind != "file")
                        .accessibilityLabel("Browse selected file's recorded containing catalog folder")
                        .help("Browse siblings by exact catalog parent ID; never open an OS path. Back returns to candidates, then the original search when applicable.")
                    Button("Find same filename (any size)") { model.searchSameFilenameAcrossDisks() }
                        .disabled(!model.pageReady || model.copySourceID != nil || entry.kind != "file")
                        .keyboardShortcut("n", modifiers: [.command, .shift])
                        .accessibilityLabel("Search exact catalog filename across all disks, ignoring recorded byte size")
                        .help("Different recorded sizes may be versions or unrelated files; no hash or content equality. Back restores search.")
                    Button("Find same byte size (any filename)") { model.searchSameRecordedSizeAcrossDisks() }
                        .disabled(!model.pageReady || model.copySourceID != nil
                            || entry.kind != "file" || (entry.size ?? 0) <= 0)
                        .keyboardShortcut("s", modifiers: [.command, .shift])
                        .accessibilityLabel("Search all catalogued disks for files of identical recorded byte count, including different names")
                        .help("Potential renamed files by size only, no hashes, USB presence or byte equality. Back restores search.")
                    Button("Find copies on other disks") { model.copiesOfSelected() }
                        .disabled(entry.kind != "file" || entry.size == nil)
                        .keyboardShortcut("d", modifiers: [.command])
                        .help("Compare exact normalized file name and recorded byte size across catalogued disks; no content hashes or deletion")
                    Button("Browse this catalog disk") { model.browseDisk(entry.volume_id) }
                        .help("Open the disk's catalogued root; does not access USB hardware")
                    Divider()
                } else {
                    Text("Select a file to identify its catalogued USB disk.")
                        .font(.callout).foregroundStyle(.secondary)
                }
    }

    @ViewBuilder private var liveMountControls: some View {
                Text("LIVE OS MOUNT IDENTITIES").font(.caption.bold()).foregroundStyle(.secondary)
                Picker("Mounted OS volume", selection: $model.selectedPhysicalID) {
                    Text("Choose a current mount").tag("")
                    ForEach(model.pairablePhysicalVolumes) { mount in
                        Text("\(mount.name) • \(mount.currentMount)").tag(mount.id)
                    }
                }
                .labelsHidden()
                .accessibilityLabel("Mounted volume for explicit physical disk pairing")
                .help("Use only after checking the actual USB sticker; filesystem UUID can be cloned")
                if model.pairablePhysicalVolumes.count < model.mountedPhysicalVolumes.count {
                    Label("Duplicate OS volume UUID: ambiguous mounts cannot be paired.", systemImage: "exclamationmark.triangle")
                        .font(.caption).foregroundStyle(.orange)
                }
                if !model.physicalProbeOK {
                    Label("OS scan incomplete: physical presence not verified.", systemImage: "exclamationmark.triangle")
                        .font(.caption).foregroundStyle(.orange)
                        .accessibilityLabel("Physical connection unverified: OS volume scan incomplete")
                }
                Button("Copy current OS root") { model.copyCurrentOSMountRoot() }
                    .disabled(model.selectedDiskForPairing == nil)
                    .accessibilityLabel("Copy current OS volume root after verifying explicit pairing")
                    .help("Recheck linked CTF SHA, OS UUID and mount; never infer a recorded file path")
                Button("Open verified disk in Finder") { model.openCurrentOSMountRoot() }
                    .disabled(model.selectedDiskForPairing == nil)
                    .accessibilityLabel("Open the currently OS-verified, explicitly paired disk root in Finder")
                    .help("Fresh CTF-generation and two OS UUID/mount probes; no guessed catalogued file path")
                Button("Refresh OS mounts") { model.refreshPhysicalPresence() }
                    .keyboardShortcut("r", modifiers: [.command])
                    .help("Recheck current OS volume identities (Command-R); never use a recorded drive letter")
                    .accessibilityLabel("Recheck physical OS volumes")
                Button("Pair selected catalog disk…") { model.pairSelectedPhysicalDisk() }
                    .disabled(model.selectedDiskForPairing == nil || model.selectedPhysicalID.isEmpty || !model.physicalProbeOK)
                Button("Remove saved pairing…") { model.unpairSelectedPhysicalDisk() }
                    .disabled(model.selectedDiskForPairing == nil)
                Text("Only a freshly OS-verified, explicitly paired UUID may show connected. A clone may share a filesystem UUID. Historical drive letters are never used.")
                    .font(.caption).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                Text("FIND THE DISK").font(.caption.bold()).foregroundStyle(.secondary)
                selectedFileDetails
                shelfTagEditor
                Button("Search all catalogued disks") { model.searchAcrossAllDisks() }
                    .disabled(model.query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .help("Run the current expression without a disk filter")
                Divider()
                liveMountControls
                Spacer(minLength: 0)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(10)
        }
        .frame(minWidth: 230, idealWidth: 270, maxWidth: 330)
        .onAppear {
            shelfDraftDiskID = model.selectedDiskForPairing
            shelfDraft = shelfDraftDiskID.map { model.shelfLabel($0) } ?? ""
        }
        // The one-argument onChange overload supports our macOS 13 deployment.
        .onChange(of: model.selectedDiskForPairing) { id in
            shelfDraftDiskID = id
            shelfDraft = id.map { model.shelfLabel($0) } ?? ""
        }
        .onChange(of: model.physicalScope) { _ in
            shelfDraftDiskID = model.selectedDiskForPairing
            shelfDraft = shelfDraftDiskID.map { model.shelfLabel($0) } ?? ""
        }
        .onChange(of: model.shelfLabels) { _ in
            if shelfDraft.isEmpty, let id = shelfDraftDiskID {
                shelfDraft = model.shelfLabel(id)
            }
        }
        .accessibilityLabel("Selected file disk locator")
    }
}

@MainActor struct CatalogView: View {
    @StateObject private var model = Catalog()
    @FocusState private var searchFieldFocused: Bool
    @FocusState private var diskFilterFocused: Bool
    @FocusState private var resultTableFocused: Bool
    @AppStorage("AtlasCatalog.locatorVisible") private var locatorVisible = true
    @AppStorage("AtlasCatalog.categoriesVisible") private var categoriesVisible = false
    @AppStorage("AtlasCatalog.compactDiskRows") private var compactDiskRows = true
    var body: some View {
        VStack(spacing: 4) {
            HStack(spacing: 6) {
                Label("OFFLINE USB CATALOG", systemImage: "externaldrive")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
                    .accessibilityLabel("Offline search; physical disk identity requires an OS probe")
                Spacer(minLength: 4)
                Menu("Catalog") {
                    Button("Open catalog…") { model.chooseCatalog(create: false) }
                        .disabled(!model.mutationAllowed)
                    Button("New catalog…") { model.chooseCatalog(create: true) }
                        .disabled(!model.mutationAllowed)
                    Divider()
                    Button("Add disk (scan)…") { model.addDisk() }
                        .disabled(!model.editableAllowed)
                    Menu("CTF sources") {
                        Button("Link read-only CTF…") { model.linkCTF() }
                        Button("Refresh linked CTF") { model.refreshLinkedCTF() }
                            .disabled(!model.linkedMode)
                        Button("Review notes and overlays…") { model.reviewAnnotations() }
                            .disabled(!model.linkedMode)
                        Divider()
                        Button("Import CTF copy into SQLite…") { model.importCTF() }
                            .disabled(!model.editableAllowed)
                    }
                    Divider()
                    Button("Check catalog integrity") { model.verifyCatalog() }
                        .disabled(!model.editableAllowed)
                    Button("Compact SQLite copy…") { model.compactCopy() }
                        .disabled(!model.editableAllowed)
                    Divider()
                    Menu("Local search latency") {
                        Button("Show verified global-search p50/p95") { model.showGlobalSearchLatency() }
                            .keyboardShortcut("8", modifiers: [.command, .shift])
                            .accessibilityLabel("Show local session latency of verified all-disk search pages")
                        Button("Reset search latency session") { model.resetGlobalSearchLatency() }
                            .keyboardShortcut("7", modifiers: [.command, .shift])
                            .accessibilityLabel("Clear private in-memory search timing samples")
                    }
                }
                .accessibilityLabel("Catalog tools, imports and maintenance")
                .help("Secondary operations; search defaults to all catalogued disks.")
                Menu("Export") {
                    Button("CSV • current scope") {
                        model.exportFormat = "csv"; model.export()
                    }.disabled(!model.mutationAllowed)
                    Button("Parquet • current scope") {
                        model.exportFormat = "parquet"; model.export()
                    }.disabled(!model.mutationAllowed)
                    Button("CTF • entire catalog") {
                        model.exportFormat = "ctf"; model.export()
                    }.disabled(!model.mutationAllowed || model.linkedMode)
                    Button("JSON • current scope") {
                        model.exportFormat = "json"; model.export()
                    }.disabled(!model.mutationAllowed)
                }
                .accessibilityLabel("Export recorded catalog metadata")
                .help("Explicit export format; linked original CTF stays read-only")
                if model.pending > 0 { ProgressView().controlSize(.small) }
                Button("Cancel scan") { model.cancel() }.disabled(!model.canCancel)
            }
            HStack(spacing: 6) {
                Button { model.back() } label: { Image(systemName: "chevron.left") }.help("Back to parent folder or leave results")
                Button { model.openSelected() } label: { Image(systemName: "folder") }.disabled(model.selected?.isFolder != true).keyboardShortcut(.downArrow, modifiers: [.command]).help("Open selected folder (Command-Down)")
                TextField("Find file or film • name:… ext:mkv AND/OR…",
                          text: Binding(
                              get: { model.query },
                              set: { model.userEditedSearchText($0) }))
                    .focused($searchFieldFocused)
                    .onSubmit { model.search() }
                    .accessibilityLabel("Search catalogued files")
                    .accessibilityHint("Plain filenames search every disk after a short pause. Press Return for fields or Boolean expressions.")
                    .help("Live all-disk filename lookup after 450 ms and 2 characters; Return executes full Boolean or field queries")
                    .task(id: model.liveFilenameTaskID) {
                        let editToken = model.liveFilenameTaskID
                        let candidate = await MainActor.run {
                            model.pendingLiveFilenameSearch(for: editToken)
                        }
                        guard let candidate else { return }
                        do {
                            try await Task.sleep(nanoseconds: 450_000_000)
                        } catch {
                            return // New typing or explicit navigation cancels the old task.
                        }
                        guard !Task.isCancelled else { return }
                        await MainActor.run {
                            model.runLiveFilenameSearchIfCurrent(candidate, editToken: editToken)
                        }
                    }
                Button { searchFieldFocused = true } label: { Image(systemName: "magnifyingglass") }
                    .keyboardShortcut("f", modifiers: [.command])
                    .help("Focus file search (Command-F)")
                Button("Search all disks") { model.searchAcrossAllDisks() }
                    .help("Default Enter/Command-F search scope: all catalogued disks")
                Button("This disk") { model.searchInSelectedDisk() }
                    .disabled(model.volumeID == nil || model.query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .help("Narrow this expression explicitly to the selected catalog disk")
                Picker("Duplicate matching", selection: $model.duplicateMode) {
                    Text("Name + size").tag("name_size"); Text("Size").tag("size"); Text("Name").tag("name")
                }.labelsHidden().frame(width: 120).accessibilityLabel("Duplicate matching mode")
                Button("Duplicates") { model.duplicates() }
            }
            HStack(spacing: 8) {
                Text("AND  OR  NOT  ( )   name:  path:  ext:  type:video")
                    .foregroundStyle(.secondary)
                Button(model.videoFilterActive ? "All formats" : "Video files") {
                    model.toggleVideoFormatSearch()
                }
                .keyboardShortcut("v", modifiers: [.command, .shift])
                .accessibilityLabel("Toggle extension-based video search across all catalogued disks")
                .help("Filename extensions only, not film recognition or content hashes (Command-Shift-V)")
                Spacer(minLength: 4)
                Text(model.resultScopeCaption).lineLimit(1).truncationMode(.middle)
                    .accessibilityLabel("Current search scope")
                Picker("Sort current page", selection: $model.pageSortColumn) {
                    Text("Source order").tag("natural")
                    Text("Name").tag("name")
                    Text("Disk").tag("disk")
                    Text("Kind").tag("kind")
                    Text("Size").tag("size")
                    Text("Modified").tag("modified")
                    Text("Candidates").tag("candidates")
                    Text("Path").tag("path")
                }
                .labelsHidden().frame(width: 122)
                .accessibilityLabel("Sort the currently loaded page only")
                .help("Sort only the 250 loaded results, never the whole catalog")
                Button(model.pageSortDescending ? "↓" : "↑") {
                    model.pageSortDescending.toggle()
                }
                .disabled(model.pageSortColumn == "natural")
                .accessibilityLabel(model.pageSortDescending ? "Descending page sort" : "Ascending page sort")
                Text("PAGE ONLY").font(.caption2).foregroundStyle(.secondary)
                Button("Results") {
                    if model.focusCurrentPageResults() { resultTableFocused = true }
                }
                .disabled(!model.pageReady || model.entries.isEmpty)
                .keyboardShortcut("e", modifiers: [.command])
                .accessibilityLabel("Focus verified loaded search results")
                .help("Focus the current results page without changing the disk search scope (Command-E)")
                // Always visible in the compact toolbar, including 880px with
                // the inspector collapsed. Keep every shortcut on its native
                // menu action, not on a conditionally hidden inspector.
                Menu("File actions") {
                    Button("Folder") { model.browseSelectedFileFolder() }
                        .disabled(!model.pageReady || model.selected?.kind != "file")
                        .keyboardShortcut("f", modifiers: [.command, .shift])
                        .accessibilityLabel("Browse recorded parent catalog folder of the selected file")
                        .help("Recorded catalog parent only; Back returns to search.")
                    Button("Full disk register") { model.copyFullDiskRegister() }
                        .disabled(model.volumes.isEmpty)
                        .keyboardShortcut("4", modifiers: [.command, .shift])
                        .accessibilityLabel("Copy every registered catalog disk ID, owner shelf tag, and last independently OS-verified status")
                        .help("FULL offline USB inventory, not the current search page or filter; linked source SHA rechecked (Command-Shift-4)")
                    Button("Disk card") { model.copySelectedDiskPickupCard() }
                        .disabled(model.selectedDiskForPairing == nil)
                        .keyboardShortcut("l", modifiers: [.command, .shift])
                        .accessibilityLabel("Copy selected catalog disk physical pickup card")
                        .help("Exact ID and private sticker, not live file evidence")
                    Divider()
                    Button("OS root") { model.copyCurrentOSMountRoot() }
                        .disabled(model.selectedDiskForPairing == nil)
                        .keyboardShortcut("h", modifiers: [.command, .shift])
                        .accessibilityLabel("Copy freshly OS-verified volume root")
                        .help("Two independent live UUID probes and linked CTF source check")
                    Button("Finder") { model.openCurrentOSMountRoot() }
                        .disabled(model.selectedDiskForPairing == nil)
                        .keyboardShortcut("b", modifiers: [.command, .shift])
                        .accessibilityLabel("Open OS-verified USB disk root in Finder")
                        .help("Two UUID/mount probes; catalogued file presence is unknown")
                }
                .accessibilityLabel("File and physical disk actions")
                .help("Contextual catalog folder, disk card and verified OS root actions")
                Button(locatorVisible ? "Hide locator" : "Show locator") {
                    locatorVisible.toggle()
                }
                .keyboardShortcut("i", modifiers: [.command, .shift])
                .accessibilityLabel("Toggle selected file disk locator")
                .help("Show or hide the native disk locator (Command-Shift-I); current search scope is unchanged")
            }.font(.caption)
            if let copyName = model.copySourceName {
                HStack(spacing: 6) {
                    Label("COPY CANDIDATES • \(copyName)", systemImage: "doc.on.doc")
                        .font(.caption.weight(.semibold)).lineLimit(1)
                    Text("Name + size only; contents not compared").font(.caption).foregroundStyle(.secondary)
                    Spacer(minLength: 6)
                    Button("Copy pickup list") { model.copyPickupPlan() }
                        .keyboardShortcut("p", modifiers: [.command, .shift])
                        .disabled(!model.pageReady || model.copyDiskGroups.isEmpty)
                        .accessibilityLabel("Copy catalogued candidate disk IDs, shelf tags, counts and last OS states")
                        .help("Local checklist; OS status does not verify file bytes (Command-Shift-P)")
                    Button("Copy page paths") { model.copyCandidatePagePaths() }
                        .keyboardShortcut("m", modifiers: [.command, .shift])
                        .disabled(!model.pageReady)
                        .accessibilityLabel("Copy recorded paths from this loaded metadata candidate page only")
                        .help("Copy at most 250 recorded paths, NOT all matches; no physical or content verification (Command-Shift-M)")
                    Button("Use verified mounted disk") { model.focusSoleConnectedCopy() }
                        .disabled(!model.pageReady || model.copyDiskGroups.isEmpty)
                        .keyboardShortcut("g", modifiers: [.command])
                        .help("Fresh OS check, then focus the sole explicitly paired and observed candidate disk (Command-G)")
                        .accessibilityLabel("Focus sole OS-verified copy disk after refreshing live mounts")
                    Button("Back to results") { model.back() }
                }.accessibilityElement(children: .contain)
                ScrollView(.horizontal) {
                    HStack(spacing: 5) {
                        Button("All disks (\(model.copyDiskGroups.reduce(0) { $0 + $1.count }))") {
                            model.focusCopyCandidates(on: nil)
                        }
                        .accessibilityLabel("Show all name and size candidate copies across catalogued disks")
                        ForEach(model.orderedCopyDiskGroups) { group in
                            Button(model.candidateCopyDiskCaption(group)) {
                                model.focusCopyCandidates(on: group.volume_id)
                            }
                            .accessibilityLabel(model.candidateCopyDiskAccessibility(group))
                            .help("Choose by exact catalog ID and your sticker/shelf tag; metadata candidates only. No physical file access or content comparison.")
                        }
                    }.buttonStyle(.bordered)
                }
                .accessibilityLabel("Copy candidate disk filters")
                .frame(height: 30)
            }
            HSplitView {
                VStack(alignment: .leading, spacing: 3) {
                    HStack(spacing: 5) {
                        Text("DISKS \(model.isPageRegisterMode ? "• PAGE ONLY " : "")• \(model.visibleVolumes.count)/\(model.volumes.count)")
                            .font(.caption.bold()).foregroundStyle(.secondary)
                        Spacer(minLength: 0)
                        Button {
                            model.showPhysicalStateInRegister("connected")
                        } label: { Image(systemName: "checkmark.circle") }
                        .keyboardShortcut("o", modifiers: [.command, .shift])
                        .accessibilityLabel("Filter register to freshly OS-verified connected catalog disks")
                        .help("Refresh OS UUIDs; show only explicitly paired connected disks (Command-Shift-O)")
                        Button {
                            model.showPhysicalStateInRegister("unverified")
                        } label: { Image(systemName: "questionmark.circle") }
                        .keyboardShortcut("u", modifiers: [.command, .shift])
                        .accessibilityLabel("Filter register to catalog disks without verified physical identity")
                        .help("Refresh OS UUIDs; show unverified disks, including scan failures (Command-Shift-U)")
                        Button("Page") { model.revealCurrentPageDisksInRegister() }
                            .disabled(!model.pageReady || model.awaitingPage || model.entries.isEmpty)
                            .keyboardShortcut("6", modifiers: [.command, .shift])
                            .accessibilityLabel("Filter register to exact disk IDs on the current verified results page only")
                            .accessibilityHint("At most 250 loaded entries, not the whole search or evidence of live file presence.")
                            .help("CURRENT PAGE ONLY disk distribution • Command-Shift-6")
                        Button("Candidates") {
                            model.revealCopyCandidateDisksInRegister()
                        }
                        .disabled(model.copySourceID == nil || model.copyDiskGroups.isEmpty)
                        .keyboardShortcut("k", modifiers: [.command, .shift])
                        .accessibilityLabel("Show only catalog disks containing name and size copy candidates")
                        .help("View-only exact disk ID filter; no USB mount or file content verification (Command-Shift-K)")
                        Button {
                            model.cycleRegisterOrder()
                        } label: { Image(systemName: "arrow.up.arrow.down") }
                        .keyboardShortcut("t", modifiers: [.command, .shift])
                        .accessibilityLabel("Cycle physical disk register order, current: \(model.registerOrderCaption)")
                        .accessibilityHint("Catalog order, owner sticker and shelf A to Z, or last independently verified OS state. Does not change search, selected file or disk pairing.")
                        .help("Register order: \(model.registerOrderCaption) (Command-Shift-T); copy candidates retain their priority")
                        Button {
                            compactDiskRows.toggle()
                        } label: {
                            Image(systemName: compactDiskRows ? "rectangle.grid.1x2" : "line.3.horizontal")
                        }
                        .keyboardShortcut("9", modifiers: [.command, .shift])
                        .accessibilityLabel(compactDiskRows ? "Show detailed disk register rows" : "Show compact disk register rows")
                        .accessibilityHint("Changes only the row height; selected catalog disk, physical pairing and search remain unchanged.")
                        .help("Toggle dense 40-disk register (Command-Shift-9)")
                        Button {
                            diskFilterFocused = true
                        } label: {
                            Image(systemName: "line.3.horizontal.decrease")
                        }
                        .keyboardShortcut("l", modifiers: [.command])
                        .accessibilityLabel("Focus disk register filter")
                        .help("Filter registered disk labels or catalog IDs (Command-L)")
                    }.padding(.horizontal, 6)
                    Text(model.registerAuditCaption)
                        .font(.caption2).foregroundStyle(.secondary)
                        .lineLimit(1).minimumScaleFactor(0.75)
                        .accessibilityLabel(model.registerAuditAccessibility)
                        .padding(.horizontal, 6)
                    HStack(spacing: 4) {
                        TextField("Filter disk label / ID", text: $model.diskFilter)
                            .focused($diskFilterFocused)
                            .accessibilityLabel("Filter catalog disk register")
                            .help("Register only: page: (loaded PAGE ONLY), id:42, copies:, state:connected/offline/conflict/unverified, tag:missing/shared. Search scope unchanged.")
                        if !model.diskFilter.isEmpty {
                            Button {
                                model.diskFilter = ""
                                diskFilterFocused = true
                            } label: {
                                Image(systemName: "xmark.circle.fill")
                            }
                            .accessibilityLabel("Clear disk register filter")
                            .help("Show all registered disks without changing search scope")
                        }
                    }.padding(.horizontal, 5)
                    if let selectedID = model.volumeID,
                       !model.visibleVolumes.contains(where: { $0.id == selectedID }) {
                        Text("Current disk scope is hidden by register filter")
                            .font(.caption2).foregroundStyle(.secondary)
                            .padding(.horizontal, 6)
                    }
                    List(selection: Binding(get: { model.volumeID }, set: { model.volumeID = $0; model.selectVolume() })) {
                        Text("All disks / global search").tag(Optional<Int64>.none)
                        ForEach(model.visibleVolumes) { disk in
                            VStack(alignment: .leading, spacing: 1) {
                                if compactDiskRows {
                                    HStack(spacing: 4) {
                                        Image(systemName: "externaldrive")
                                            .foregroundStyle(.secondary)
                                        Text(model.isPageRegisterMode
                                             ? "#\(disk.id) • \(model.pageRegisterCount(disk.id)) THIS-PAGE hits • \(disk.label)"
                                             : "#\(disk.id) • \(disk.label)")
                                            .lineLimit(1)
                                        if model.shelfConflictCaption(disk.id) != nil {
                                            Image(systemName: "exclamationmark.triangle.fill")
                                                .foregroundStyle(.orange)
                                                .help(model.shelfConflictCaption(disk.id) ?? "Shared sticker")
                                        }
                                        Spacer(minLength: 1)
                                        Text(model.isCandidateRegisterMode
                                            ? "\(model.candidateRegisterCount(disk.id)) candidates • \(model.candidateCopyOSCaption(disk.id))"
                                            : "\(model.shelfLabel(disk.id).isEmpty ? "" : model.shelfLabel(disk.id) + " • ")\(model.physicalCaption(disk.id))")
                                            .font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                                    }
                                } else {
                                    Label(model.isPageRegisterMode
                                          ? "#\(disk.id) • \(model.pageRegisterCount(disk.id)) CURRENT PAGE hits • \(disk.label)"
                                          : (model.isCandidateRegisterMode
                                          ? "#\(disk.id) • \(model.candidateRegisterCount(disk.id)) name+size candidates • \(disk.label)"
                                          : "#\(disk.id) • \(disk.label)"), systemImage: "externaldrive")
                                    if model.shelfConflictCaption(disk.id) != nil {
                                        Text(model.shelfConflictCaption(disk.id) ?? "SHARED STICKER")
                                            .font(.caption2).foregroundStyle(.orange)
                                            .accessibilityLabel(model.shelfConflictCaption(disk.id) ?? "SHARED STICKER")
                                    }
                                    Text(model.isCandidateRegisterMode
                                        ? "\(model.shelfLabel(disk.id).isEmpty ? "No sticker" : "Shelf: " + model.shelfLabel(disk.id)) • \(model.candidateCopyOSCaption(disk.id)) • content unverified"
                                        : "\(model.shelfLabel(disk.id).isEmpty ? "" : "Shelf: " + model.shelfLabel(disk.id) + " • ")\(disk.entry_count ?? 0) items • \(model.physicalCaption(disk.id))")
                                        .font(.caption).foregroundStyle(.secondary).lineLimit(1)
                                }
                            }
                            .tag(Optional(disk.id))
                            .accessibilityElement(children: .combine)
                            .accessibilityLabel(model.isPageRegisterMode
                                ? model.pageRegisterAccessibility(disk)
                                : (model.isCandidateRegisterMode
                                ? model.candidateRegisterAccessibility(disk)
                                : "Catalog disk #\(disk.id), \(disk.label), \(model.physicalCaption(disk.id))"))
                            .help(model.isCandidateRegisterMode
                                ? "Metadata name+size candidate disk. Last OS check is not proof the file exists."
                                : (disk.root_path ?? disk.label))
                        }
                    }.listStyle(.sidebar)
                    Button(categoriesVisible ? "Hide category tools" : "Show category tools") {
                        categoriesVisible.toggle()
                    }
                    .font(.caption)
                    .buttonStyle(.borderless)
                    .keyboardShortcut("a", modifiers: [.command, .shift])
                    .accessibilityLabel(categoriesVisible ? "Collapse category administration" : "Expand category administration")
                    .accessibilityHint("The disk register gets the available height; search and disk scope never change.")
                    .help("Secondary category tools • Command-Shift-A; keeps all ~40 catalog disks easy to scan")
                    .padding(.horizontal, 6)
                    if categoriesVisible {
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
                    }
                }.frame(minWidth: 190, idealWidth: 245, maxWidth: 360)
                Table(model.displayEntries, selection: $model.selection) {
                    TableColumn("Name") { entry in Label(entry.name, systemImage: entry.isFolder ? "folder" : "doc").lineLimit(1) }.width(min: 145, ideal: 230)
                    TableColumn("Disk") { entry in
                        Text(model.resultDiskCaption(entry.volume_id))
                            .lineLimit(1)
                            .help(model.resultDiskAccessibility(entry.volume_id))
                            .accessibilityLabel(model.resultDiskAccessibility(entry.volume_id))
                    }.width(min: 90, ideal: 155)
                    TableColumn("Kind") { Text($0.kind) }.width(66)
                    TableColumn("Size") { Text($0.sizeText).monospacedDigit() }.width(83)
                    TableColumn("Modified") { Text($0.dateText) }.width(108)
                    TableColumn("Matches") { Text($0.duplicate_count.map(String.init) ?? "") }.width(60)
                    TableColumn("Path", value: \.relative_path).width(min: 115, ideal: 180)
                }.focused($resultTableFocused)
                .onTapGesture(count: 2) { model.openSelected() }
                .contextMenu {
                    // The table-level context menu explicitly targets its
                    // CURRENT selected row; selection is never inferred from
                    // an old list, a historical drive letter or a mount path.
                    if model.pageReady, !model.awaitingPage, let row = model.selected {
                        Button("Open selected catalog folder") { model.openSelected() }
                            .disabled(!row.isFolder)
                        Button("Browse selected file's recorded parent") { model.browseSelectedFileFolder() }
                            .disabled(row.kind != "file")
                        Button("Find name + size candidates on other disks") { model.copiesOfSelected() }
                            .disabled(row.kind != "file" || row.size == nil)
                        Button("Find exact filename across all disks") { model.searchSameFilenameAcrossDisks() }
                            .disabled(row.kind != "file" || model.copySourceID != nil)
                        Button("Find same recorded size across disks") { model.searchSameRecordedSizeAcrossDisks() }
                            .disabled(row.kind != "file" || (row.size ?? 0) <= 0 || model.copySourceID != nil)
                        Divider()
                        Button("Show exact catalog disk in register") { model.revealSelectedDiskInRegister() }
                        Button("Copy catalog retrieval note") {
                            model.copySelectedMetadata(
                                model.retrievalNote(for: row), entryID: row.id, diskID: row.volume_id)
                        }
                        Button("Copy physical disk pickup card") { model.copySelectedDiskPickupCard() }
                    } else {
                        Text("Select a verified current result before using file actions")
                    }
                }
                if locatorVisible {
                    DiskLocatorInspector(model: model)
                }
            }
            HStack(spacing: 8) {
                Text(model.status).lineLimit(1).truncationMode(.middle).textSelection(.enabled)
                Spacer()
                if model.awaitingPage {
                    ProgressView().controlSize(.small)
                        .accessibilityLabel("Catalog page request is still running")
                }
                Button("Stop waiting") { model.stopWaitingForPage() }
                    .disabled(!model.awaitingPage)
                    .keyboardShortcut("x", modifiers: [.command, .shift])
                    .accessibilityLabel("Stop waiting and discard the pending catalog page")
                    .help("Does not interrupt the source reader; its eventual reply is discarded (Command-Shift-X)")
                Button("Retry page") { model.retryCurrentPage() }
                    .disabled(!model.retryPageAvailable)
                    .keyboardShortcut("y", modifiers: [.command, .shift])
                    .accessibilityLabel("Retry dismissed or failed catalog page in the same scope")
                    .help("Rechecks linked CTF generation SHA for a new page (Command-Shift-Y)")
                Text(model.pageReady ? "Page \(model.offset / model.limit + 1)" : "Page not verified")
                    .font(.caption2).foregroundStyle(.secondary)
                    .accessibilityLabel(model.pageReady
                        ? "Verified catalog page \(model.offset / model.limit + 1)"
                        : "No verified catalog page")
                Button("Previous") { model.moveResultsPage(forward: false) }
                    .disabled(!model.canPreviousResultsPage)
                    .keyboardShortcut(.leftArrow, modifiers: [.command, .option])
                    .accessibilityLabel("Previous verified catalog results page")
                    .help("Fresh source recheck; Command-Option-Left Arrow")
                Button("Next") { model.moveResultsPage(forward: true) }
                    .disabled(!model.canNextResultsPage)
                    .keyboardShortcut(.rightArrow, modifiers: [.command, .option])
                    .accessibilityLabel("Next verified catalog results page")
                    .help("Fresh source recheck; Command-Option-Right Arrow")
            }.font(.caption)
            Text(model.linkedMode ? "LINKED CTF (read-only) • " + (model.linkedCTFPath ?? "") : model.database).font(.caption2).foregroundStyle(.secondary).frame(maxWidth: .infinity, alignment: .leading).lineLimit(1).truncationMode(.middle)
        }
        .padding(7).controlSize(.small).frame(minWidth: locatorVisible ? 1100 : 880, minHeight: 550)
        .onAppear { searchFieldFocused = true; model.refreshPhysicalPresence() }
        .onChange(of: model.restoredSelectionNonce) { _ in
            // Focus the newly verified selected row, never a cached old row.
            resultTableFocused = true
        }
        .onChange(of: model.pageReady) { ready in
            if !ready { resultTableFocused = false }
        }
        .onReceive(Timer.publish(every: 6, on: .main, in: .common).autoconnect()) { _ in
            model.refreshPhysicalPresence()
        }
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
            model.refreshPhysicalPresence()
        }
        .onReceive(NSWorkspace.shared.notificationCenter.publisher(for: NSWorkspace.didWakeNotification)) { _ in
            model.refreshPhysicalPresence()
        }
        .onReceive(NSWorkspace.shared.notificationCenter.publisher(for: NSWorkspace.willUnmountNotification)) { _ in
            model.invalidatePhysicalPresenceAfterMountEvent()
        }
        .onReceive(NSWorkspace.shared.notificationCenter.publisher(for: NSWorkspace.didUnmountNotification)) { _ in
            model.recheckPhysicalPresenceAfterMountEvent()
        }
        .onReceive(NSWorkspace.shared.notificationCenter.publisher(for: NSWorkspace.didMountNotification)) { _ in
            model.recheckPhysicalPresenceAfterMountEvent()
        }
        .onReceive(NSWorkspace.shared.notificationCenter.publisher(for: NSWorkspace.didRenameVolumeNotification)) { _ in
            // Current mount path can change without a UUID change.
            model.recheckPhysicalPresenceAfterMountEvent()
        }
        .sheet(isPresented: $model.showingAnnotationReview) {
            if model.showingVerifiedOverlay {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Verified active annotations").font(.headline)
                    Text("Independent SHA-pinned read • no writes • not the historical activation audit")
                        .font(.caption).foregroundStyle(.secondary)
                    if model.verifiedOverlayLoading {
                        ProgressView("Rechecking source generation and old-note revisions…")
                    } else if let page = model.verifiedOverlay, page.isDisplaySafe(expectedOffset: model.verifiedOverlayOffset) {
                        Text("Valid live overlays: \(page.total) • stale/conflicting active audits excluded: \(page.conflicts)")
                            .font(.subheadline).monospacedDigit()
                        Text("Unconfirmed or unresolved mappings are excluded; this API does not count them.")
                            .font(.caption).foregroundStyle(.secondary)
                        if page.state == "uninitialized" || page.state == "no_cross_generation_activations" {
                            Text("No approved cross-generation notes are active.")
                        } else {
                            Text("Rows \(page.items.isEmpty ? 0 : model.verifiedOverlayOffset + 1)–\(model.verifiedOverlayOffset + page.items.count) of \(page.total)")
                                .font(.caption).monospacedDigit()
                            List(page.items) { item in
                                HStack(alignment: .top, spacing: 12) {
                                    Text("Old #\(item.old_entry_id) → #\(item.entry_id)")
                                        .monospacedDigit().frame(width: 180, alignment: .leading)
                                    Text(String(item.note.prefix(160)))
                                        .lineLimit(2)
                                    Spacer(minLength: 4)
                                    Text("rev \(item.revision)").foregroundStyle(.secondary)
                                }.font(.caption)
                            }
                        }
                    } else {
                        Text("Verified view unavailable or invalid; no private notes displayed.")
                    }
                    Spacer(minLength: 2)
                    HStack {
                        Button("Previous") {
                            model.loadVerifiedOverlayPage(offset: max(0, model.verifiedOverlayOffset - model.annotationPageSize))
                        }.disabled(!model.verifiedOverlayHasPrevious || model.verifiedOverlayLoading)
                        Button("Next") {
                            model.loadVerifiedOverlayPage(offset: model.verifiedOverlayOffset + model.annotationPageSize)
                        }.disabled(!model.verifiedOverlayHasNext || model.verifiedOverlayLoading)
                        Spacer()
                        Button("Back to conflicts") { model.backToAnnotationConflicts() }
                            .disabled(model.verifiedOverlayLoading)
                        Button("Close") { model.resetAnnotationReview() }
                            .keyboardShortcut(.cancelAction)
                            .disabled(model.verifiedOverlayLoading)
                    }
                }.padding(14).frame(width: 820, height: 520)
            } else if let preview = model.annotationReview {
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
                    Text("The CTF and derived cache remain read-only. Approved mapping, activation and revocation actions write only to the separate user-owned notes database.")
                        .font(.caption).foregroundStyle(.secondary)
                    HStack {
                        Button("Previous") {
                            model.loadAnnotationReviewPage(offset: max(0, model.annotationReviewOffset - model.annotationPageSize))
                        }.disabled(!model.annotationReviewHasPrevious || model.annotationReviewLoading || model.annotationConfirmationBusy)
                        Button("Next") {
                            model.loadAnnotationReviewPage(offset: model.annotationReviewOffset + model.annotationPageSize)
                        }.disabled(!model.annotationReviewHasNext || model.annotationReviewLoading || model.annotationConfirmationBusy)
                        Spacer()
                        Button("Verified live notes…") { model.loadVerifiedOverlayPage(offset: 0) }
                            .disabled(model.annotationReviewLoading || model.annotationConfirmationBusy)
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
