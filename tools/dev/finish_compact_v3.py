#!/usr/bin/env python3
"""Adapt a schema-specific assertion; preserve the original nonempty-stats gate."""
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
