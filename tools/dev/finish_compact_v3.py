#!/usr/bin/env python3
"""Wire new maintenance operations and adapt a physical-table-specific assertion."""
from pathlib import Path

root = Path(__file__).resolve().parents[2]
p = root / "crates/atlas-core/tests/catalog.rs"
s = p.read_text(encoding="utf-8")
old = "SELECT COUNT(*) FROM sqlite_stat1 WHERE tbl='entries'"
new = "SELECT COUNT(*) FROM sqlite_stat1 WHERE tbl='entries_core'"
if old in s:
    if s.count(old) != 1:
        raise RuntimeError("Unexpected planner-test source")
    p.write_text(s.replace(old, new), encoding="utf-8")
elif new not in s:
    raise RuntimeError("Planner-statistics assertion missing")
p = root / "crates/atlas-service/src/lib.rs"
s = p.read_text(encoding="utf-8")
if "mod maintenance;" not in s:
    if s.count("mod legacy;") != 1:
        raise RuntimeError("Service module anchor changed")
    s = s.replace("mod legacy;", "mod legacy;\nmod maintenance;")
    marker = "    let mut data = match op {"
    if s.count(marker) != 1:
        raise RuntimeError("Service dispatcher anchor changed")
    s = s.replace(marker, marker + '\n        "compact_catalog" => maintenance::compact_copy(&v)?,\n        "validate_catalog" => maintenance::validate(&v)?,')
    p.write_text(s, encoding="utf-8")
p = root / "crates/atlas-service/src/maintenance.rs"
s = p.read_text(encoding="utf-8")
s = s.replace("use rusqlite::{params, Connection, OpenFlags, OptionalExtension};", "use rusqlite::{Connection, OpenFlags};")
p.write_text(s, encoding="utf-8")
