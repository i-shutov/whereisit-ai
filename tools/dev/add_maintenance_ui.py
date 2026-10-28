#!/usr/bin/env python3
"""One-time checked native UI patch. No database access or binary execution."""
from pathlib import Path
root = Path(__file__).resolve().parents[2]

def replace(text, old, new, count=1):
    if text.count(old) != count:
        raise RuntimeError("Native UI source changed: " + old[:80])
    return text.replace(old, new)

p = root / "apps/windows/src/native.rs"
s = p.read_text(encoding="utf-8")
if "const COMPACT_COPY:" not in s:
    s = replace(s, "const OPEN: usize = 101;", "const COMPACT_COPY: usize = 200;\nconst VALIDATE_CATALOG: usize = 201;\nconst OPEN: usize = 101;")
    s = replace(s, 'if reply.tag == "mutation" {', 'if matches!(reply.tag, "mutation" | "maintenance") {')
    marker = '                "mutation" => {'
    s = replace(s, marker, '''                "maintenance" => {
                    self.status("Catalog verification completed. The original catalog was not replaced.");
                    MessageBoxW(self.hwnd, wide(&serde_json::to_string_pretty(data).unwrap_or_default()).as_ptr(), wide("Catalog maintenance result").as_ptr(), MB_OK);
                }
''' + marker)
    marker = "            CATEGORY_FILTER | CATEGORY_ASSIGN => {"
    s = replace(s, marker, '''            COMPACT_COPY | VALIDATE_CATALOG if self.job.is_none() && self.pending == 0 => {
                let request = if command == COMPACT_COPY {
                    let Some(path) = file_dialog(self.hwnd, true, "sqlite") else { return; };
                    json!({"op":"compact_catalog","path":path})
                } else {
                    json!({"op":"validate_catalog"})
                };
                self.job = Some("maintenance".into());
                self.status("Verifying catalog / writing a compact copy. Original stays unchanged...");
                self.call("maintenance", request);
            }
''' + marker)
    marker = "        let mut message: MSG = std::mem::zeroed();"
    s = replace(s, marker, '''        let menu = CreateMenu();
        let maintenance = CreatePopupMenu();
        if !menu.is_null() && !maintenance.is_null() {
            AppendMenuW(maintenance, MF_STRING, VALIDATE_CATALOG, wide("Verify catalog").as_ptr());
            AppendMenuW(maintenance, MF_STRING, COMPACT_COPY, wide("Create compact copy...").as_ptr());
            AppendMenuW(menu, MF_POPUP, maintenance as usize, wide("Catalog").as_ptr());
            SetMenu(hwnd, menu);
            DrawMenuBar(hwnd);
        }
''' + marker)
    s = s.replace('wide("WhereIsIt AI").as_ptr()', 'wide(concat!("WhereIsIt AI ", env!("CARGO_PKG_VERSION"))).as_ptr()')
    p.write_text(s, encoding="utf-8")

p = root / "apps/macos/Sources/AtlasCatalog/AtlasCatalog.swift"
s = p.read_text(encoding="utf-8")
if "func compactCopy()" not in s:
    marker = "    func mutate(_ args: [String: Any], label: String) {"
    methods = '''    func verifyCatalog() {
        guard mutationAllowed else { return }
        job = "maintenance"
        status = "Verifying catalog without modifying it..."
        request(["op": "validate_catalog"], as: EmptyReply.self) { [weak self] result in
            guard let self else { return }
            self.job = nil
            if result != nil { self.status = "Catalog verification passed. Original data unchanged." }
        }
    }
    func compactCopy() {
        guard mutationAllowed else { return }
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
'''
    s = replace(s, marker, methods + marker)
    marker = '                Button("New…") { model.chooseCatalog(create: true) }.disabled(!model.mutationAllowed)'
    s = replace(s, marker, marker + '\n                Button("Check") { model.verifyCatalog() }.disabled(!model.mutationAllowed)\n                Button("Compact copy…") { model.compactCopy() }.disabled(!model.mutationAllowed)')
    p.write_text(s, encoding="utf-8")
print("Native maintenance UI prepared")
