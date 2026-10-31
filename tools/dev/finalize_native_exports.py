#!/usr/bin/env python3
"""One-shot, exact-precondition repair of final native export wiring."""
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2]


def replace(text, old, new, count=1):
    if text.count(old) != count:
        raise RuntimeError('native_export_patch_precondition')
    return text.replace(old, new)


win = ROOT/'apps/windows/src/native.rs'
s = win.read_text()
s = replace(s, 'fn wide(s: &str) -> Vec<u16> {', '''fn linked_export_allowed(index: isize) -> bool {
    matches!(index, 0 | 1 | 3)
}

#[cfg(test)]
mod linked_export_policy_tests {
    use super::linked_export_allowed;
    #[test]
    fn csv_parquet_json_enabled_but_ctf_and_unknown_refused() {
        for index in [0, 1, 3] {
            assert!(linked_export_allowed(index));
        }
        for index in [-1, 2, 4, isize::MAX] {
            assert!(!linked_export_allowed(index));
        }
    }
}

fn wide(s: &str) -> Vec<u16> {''')
s = replace(s, '''                            || (*command == EXPORT
                                && !self.duplicates
                                && SendMessageW(self.export_format, CB_GETCURSEL, 0, 0) != 2)))''', '''                            || (*command == EXPORT
                                && linked_export_allowed(SendMessageW(self.export_format, CB_GETCURSEL, 0, 0)))))''')
s = replace(s, '''                if self.linked_mode() && command == EXPORT {
                    if self.duplicates {
                        self.status(
                            "Linked duplicate candidates cannot be exported until a verified duplicate report is implemented.",
                        );
                        return;
                    }
                    if SendMessageW(self.export_format, CB_GETCURSEL, 0, 0) == 2 {
                        self.status(
                            "Linked CTF supports only CSV/Parquet export; original stays read-only.",
                        );
                        return;
                    }
                }''', '''                if self.linked_mode() && command == EXPORT
                    && !linked_export_allowed(SendMessageW(self.export_format, CB_GETCURSEL, 0, 0))
                {
                    self.status("Linked CTF supports CSV/Parquet/JSON export; original stays read-only.");
                    return;
                }''')
s = replace(s, '                            1 => "parquet",\n                            _ => "ctf",', '                            1 => "parquet",\n                            3 => "json",\n                            _ => "ctf",')
s = replace(s, '                                    1 => "export_parquet",\n                                    _ => "export_ctf",', '                                    1 => "export_parquet",\n                                    3 => "export_json",\n                                    _ => "export_ctf",')
s = replace(s, '''                                linked["format"] = json!(if operation == "export_csv" {
                                    "csv"
                                } else {
                                    "parquet"
                                });''', '''                                linked["format"] = json!(match operation {
                                    "export_csv" => "csv",
                                    "export_parquet" => "parquet",
                                    "export_json" => "json",
                                    _ => "ctf",
                                });''')
s = replace(s, '                "CTF — entire catalog",', '                "CTF — entire catalog",\n                "JSON — current branch/query",')
s = replace(s, '"parquet"=>"Apache Parquet (*.parquet)\\0*.parquet\\0All files\\0*.*\\0",', '"parquet"=>"Apache Parquet (*.parquet)\\0*.parquet\\0All files\\0*.*\\0","json"=>"JSON table (*.json)\\0*.json\\0All files\\0*.*\\0",')
win.write_text(s)
mac = ROOT/'apps/macos/Sources/AtlasCatalog/AtlasCatalog.swift'
s = mac.read_text()
s = replace(s, 'guard !linkedMode || exportFormat != "ctf" else {', 'guard !linkedMode || ["csv", "parquet", "json"].contains(exportFormat) else {')
s = replace(s, 'Linked CTF supports CSV/Parquet only; the original is read-only.', 'Linked CTF supports CSV/Parquet/JSON; the original is read-only.')
s = replace(s, '        let chosenFormat = exportFormat\n', '''        let chosenFormat = exportFormat
        let selectedMode = showingDuplicates ? "duplicates" : (appliedQuery.isEmpty && volumeID != nil ? "children" : "search")
        let selectedQuery = appliedQuery
        let selectedDuplicateMode = appliedDuplicateMode
        let selectedVolume = volumeID
        let selectedParent = stack.last.flatMap { $0.0 }
''')
s = replace(s, '(showingDuplicates ? "Export all matching duplicate candidates"', '(selectedMode == "duplicates" ? "Export all matching duplicate candidates"')
s = replace(s, 'args["mode"] = self.showingDuplicates ? "duplicates" : (self.appliedQuery.isEmpty && self.volumeID != nil ? "children" : "search")', 'args["mode"] = selectedMode')
s = replace(s, 'args["query"] = self.appliedQuery', 'args["query"] = selectedQuery')
s = replace(s, 'args["duplicate_mode"] = self.appliedDuplicateMode', 'args["duplicate_mode"] = selectedDuplicateMode')
s = replace(s, 'if let volumeID = self.volumeID { args["volume_id"] = volumeID }', 'if let selectedVolume { args["volume_id"] = selectedVolume }')
s = replace(s, '''                if !self.showingDuplicates {
                    args["parent_id"] = self.stack.last.flatMap { $0.0 }.map { $0 as Any } ?? NSNull()
                }''', '''                if selectedMode != "duplicates" {
                    args["parent_id"] = selectedParent.map { $0 as Any } ?? NSNull()
                }''')
s = replace(s, 'Text("CTF • entire catalog").tag("ctf")', 'Text("CTF • entire catalog").tag("ctf"); Text("JSON • current scope").tag("json")')
s = replace(s, '(model.exportFormat == "ctf" || model.showingDuplicates)', 'model.exportFormat == "ctf"')
mac.write_text(s)
print('Native duplicate + JSON export wiring restored; source CTF write guard preserved')
