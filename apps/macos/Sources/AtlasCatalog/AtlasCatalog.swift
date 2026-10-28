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
        request(["op": "volumes"], as: [Volume].self) { [weak self] values in
            guard let self, let values else { return }
            self.volumes = values
            self.status = "\(values.count) disks • choose a disk or search all disks"
        }
    }
    func refreshCategories() {
        request(["op": "categories"], as: [Category].self) { [weak self] values in if let values { self?.categories = values } }
    }
    func createCategory() {
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
        guard let selection, let categoryID else { return }
        request(["op": "assign_category", "entry_id": selection, "category_id": categoryID], as: EmptyReply.self) { [weak self] result in if result != nil { self?.status = "Category assigned to selected entry." } }
    }
    func selectVolume() { stack = [(nil, "/")]; offset = 0; showingDuplicates = false; load() }
    func load() {
        generation += 1; let token = generation; selection = nil
        var args: [String: Any] = ["offset": offset, "limit": limit]
        if let volumeID { args["volume_id"] = volumeID }
        if showingDuplicates {
            args["op"] = "duplicates"; args["mode"] = appliedDuplicateMode; args["query"] = appliedQuery
        } else if !appliedQuery.isEmpty {
            args["op"] = "search_fast"; args["query"] = appliedQuery
        } else if volumeID != nil {
            args["op"] = "children"; args["parent_id"] = stack.last.flatMap { $0.0 }.map { $0 as Any } ?? NSNull()
        } else { entries = []; total = 0; hasMore = false; status = "Choose a disk or enter an expression to search all disks."; return }
        status = showingDuplicates ? "Finding duplicate candidates by metadata…" : "Loading…"
        request(args, as: EntryPage.self, generationToken: token) { [weak self] page in
            guard let self, self.generation == token, let page else { return }
            self.entries = page.items; self.total = page.total ?? (self.offset + page.items.count)
            self.hasMore = page.has_more ?? (self.offset + page.items.count < self.total)
            self.status = "\(page.items.isEmpty ? 0 : self.offset + 1)–\(self.offset + page.items.count)" + (page.total.map { " of \($0) items" } ?? (self.hasMore ? " items • more available" : " items")) + (self.showingDuplicates ? " • metadata candidates; content not verified" : "")
        }
    }
    func search() { appliedQuery = query.trimmingCharacters(in: .whitespacesAndNewlines); showingDuplicates = false; offset = 0; load() }
    func duplicates() { appliedQuery = query.trimmingCharacters(in: .whitespacesAndNewlines); appliedDuplicateMode = duplicateMode; showingDuplicates = true; offset = 0; load() }
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
    func chooseCatalog(create: Bool) {
        if create {
            let panel = NSSavePanel(); panel.title = "Create Atlas catalog"; panel.nameFieldStringValue = "Catalog.atlas.sqlite"
            panel.begin { [weak self] response in if response == .OK, let url = panel.url { self?.database = url.path; self?.initialize() } }
        } else {
            let panel = NSOpenPanel(); panel.title = "Open Atlas SQLite catalog"; panel.canChooseDirectories = false; panel.allowsMultipleSelection = false
            panel.begin { [weak self] response in if response == .OK, let url = panel.url { self?.database = url.path; self?.initialize() } }
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
        let panel = NSOpenPanel(); panel.title = "Import WhereIsIt CTF"; panel.canChooseDirectories = false
        panel.begin { [weak self] response in
            guard let self, response == .OK, let url = panel.url else { return }
            self.job = "import"; self.mutate(["op": "import_ctf", "path": url.path], label: "Importing CTF…")
        }
    }
    func export() {
        let panel = NSSavePanel(); panel.title = exportFormat == "ctf" ? "Export entire catalog as CTF" : "Export current branch or query results"
        panel.nameFieldStringValue = "Catalog.\(exportFormat)"
        panel.begin { [weak self] response in
            guard let self, response == .OK, let url = panel.url else { return }
            var args: [String: Any] = ["op": "export_\(self.exportFormat)", "path": url.path, "overwrite": true]
            if self.exportFormat != "ctf" {
                args["mode"] = self.showingDuplicates ? "duplicates" : (self.appliedQuery.isEmpty && self.volumeID != nil ? "children" : "search")
                args["query"] = self.appliedQuery; args["duplicate_mode"] = self.appliedDuplicateMode
                if let volumeID = self.volumeID { args["volume_id"] = volumeID }
                args["parent_id"] = self.stack.last.flatMap { $0.0 }.map { $0 as Any } ?? NSNull(); args["recursive"] = true
            }
            self.job = "export"; self.mutate(args, label: "Exporting \(self.exportFormat.uppercased())…")
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
                Divider().frame(height: 16)
                Button("Add disk…") { model.addDisk() }.disabled(!model.mutationAllowed)
                Button("Import CTF…") { model.importCTF() }.disabled(!model.mutationAllowed)
                Picker("Export format", selection: $model.exportFormat) {
                    Text("CSV • current scope").tag("csv"); Text("Parquet • current scope").tag("parquet"); Text("CTF • entire catalog").tag("ctf")
                }.labelsHidden().frame(width: 174).accessibilityLabel("Export format and scope")
                Button("Export…") { model.export() }.disabled(!model.mutationAllowed)
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
                        TextField("New category", text: $model.categoryName).onSubmit { model.createCategory() }
                        Button("+") { model.createCategory() }.disabled(model.categoryName.trimmingCharacters(in: .whitespaces).isEmpty).help("Create category")
                    }.padding(.horizontal, 5)
                    HStack(spacing: 4) {
                        Button("Filter") { model.filterCategory() }.disabled(model.categoryID == nil)
                        Button("Assign selected") { model.assignCategory() }.disabled(model.selection == nil || model.categoryID == nil)
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
            Text(model.database).font(.caption2).foregroundStyle(.secondary).frame(maxWidth: .infinity, alignment: .leading).lineLimit(1).truncationMode(.middle)
        }
        .padding(7).controlSize(.small).frame(minWidth: 960, minHeight: 500)
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
