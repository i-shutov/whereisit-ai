#!/usr/bin/env python3
"""Checked final safety patch; no user catalog is opened by this script."""
from pathlib import Path
root = Path(__file__).resolve().parents[2]
p = root / "crates/atlas-service/src/maintenance.rs"
s = p.read_text(encoding="utf-8")
if "fn same_existing_file(" not in s:
    marker = "fn destination(source: &Path, target: &Path) -> Result<PathBuf> {"
    helper = '''fn same_existing_file(a: &Path, b: &Path) -> bool {
    let (Ok(a), Ok(b)) = (a.canonicalize(), b.canonicalize()) else { return false; };
    if a == b || a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase() { return true; }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(a), Ok(b)) = (fs::metadata(a), fs::metadata(b)) {
            return a.dev() == b.dev() && a.ino() == b.ino();
        }
    }
    false
}

'''
    if s.count(marker) != 1 or s.count("if result == protected {") != 1:
        raise RuntimeError("Destination safety source changed")
    s = s.replace(marker, helper + marker)
    s = s.replace("if result == protected {", "if result == protected || result.to_string_lossy().to_lowercase() == protected.to_string_lossy().to_lowercase() {")
    marker = "    for base in [source_real, source_alias] {"
    if s.count(marker) != 1:
        raise RuntimeError("Protected-path source changed")
    extra = '''    // A case-insensitive or Unicode-normalizing filesystem may resolve an
    // alternate base spelling to the same catalog even when its sidecar is absent.
    let target_name = name.to_string_lossy();
    for suffix in ["-wal", "-shm", "-journal"] {
        if target_name.to_ascii_lowercase().ends_with(suffix) {
            let stem = &target_name[..target_name.len() - suffix.len()];
            if same_existing_file(source, &parent.join(stem)) {
                return Err(Error::new("unsafe_output", "Output aliases a source SQLite sidecar"));
            }
        }
    }
'''
    s = s.replace(marker, extra + marker)
    p.write_text(s, encoding="utf-8")

p = root / "crates/atlas-service/tests/maintenance.rs"
s = p.read_text(encoding="utf-8")
if "uppercase_sidecar_aliases_are_rejected" not in s:
    s += '''
#[test]
fn uppercase_sidecar_aliases_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.sqlite");
    drop(old_catalog(&source, 1));
    let before = std::fs::read(&source).unwrap();
    for name in ["source.sqlite-WAL", "source.sqlite-SHM", "SOURCE.SQLITE-journal"] {
        let output = dir.path().join(name);
        let report = call(json!({"op":"compact_catalog","db":source,"path":output}));
        assert_eq!(report["ok"], false, "{report}");
        assert!(!output.exists());
    }
    assert_eq!(std::fs::read(&source).unwrap(), before);
}
'''
    p.write_text(s, encoding="utf-8")

p = root / "tools/storage_audit.py"
s = p.read_text(encoding="utf-8")
if "from contextlib import closing" not in s:
    s = s.replace("import argparse\n", "import argparse\nfrom contextlib import closing\nimport unicodedata\n")
    s = s.replace('"fixture #?.sqlite"', '"fixture # percent.sqlite"')
    s = s.replace("with connect_readonly(source) as conn:", "with closing(connect_readonly(source)) as conn:")
    marker = "    if path.resolve() in reserved or path.is_symlink():"
    if s.count(marker) != 1:
        raise RuntimeError("Profiler output guard changed")
    s = s.replace(marker, '''    normalized = lambda value: unicodedata.normalize("NFC", str(value)).casefold()
    reserved_normalized = {normalized(value) for value in reserved}
    if normalized(path.resolve()) in reserved_normalized or path.is_symlink():''')
    p.write_text(s, encoding="utf-8")
print("Sidecar aliases and cross-platform profiler cleanup hardened")
