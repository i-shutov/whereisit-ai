#!/usr/bin/env python3
"""Read-only, aggregate-only storage profile of a WhereIsIt AI SQLite catalog.

Does not export names, paths, descriptions, tags, or CTF opaque bytes.
No optional Python packages. Run on a dedicated, immutable copy of the catalog.
"""
from __future__ import annotations

import argparse
import json
import sqlite3
import sys
from pathlib import Path
from urllib.parse import quote

SQL = {
    "entry_fields": """
        SELECT COUNT(*) AS records,
               COUNT(NULLIF(description,'')) AS descriptions_nonempty,
               COUNT(NULLIF(tags,'')) AS tags_nonempty,
               COALESCE(SUM(LENGTH(CAST(description AS BLOB))),0) AS description_bytes,
               COALESCE(SUM(LENGTH(CAST(tags AS BLOB))),0) AS tag_bytes,
               COALESCE(SUM(LENGTH(CAST(relative_path AS BLOB))),0) AS path_bytes,
               COALESCE(SUM(LENGTH(CAST(name AS BLOB))),0) AS name_bytes,
               COALESCE(SUM(LENGTH(CAST(search_text AS BLOB))),0) AS search_text_bytes,
               COALESCE(SUM(LENGTH(CAST(name_norm AS BLOB))),0) AS normalized_name_bytes
        FROM entries
    """,
    "legacy_fields": """
        SELECT COUNT(*) AS records,
               COALESCE(SUM(CASE WHEN LENGTH(opaque)>0 THEN 1 ELSE 0 END),0) AS opaque_nonempty,
               COALESCE(SUM(LENGTH(opaque)),0) AS opaque_bytes,
               COALESCE(MAX(LENGTH(opaque)),0) AS max_opaque_bytes
        FROM ctf_entries
    """,
    "entry_payload_distribution": """
        SELECT CASE
                 WHEN description<>'' AND tags<>'' THEN 'description_and_tags'
                 WHEN description<>'' THEN 'description_only'
                 WHEN tags<>'' THEN 'tags_only'
                 ELSE 'empty'
               END AS kind,COUNT(*)
        FROM entries GROUP BY kind
    """,
    "legacy_payload_distribution": """
        SELECT CASE
                 WHEN LENGTH(opaque)=0 THEN 'empty'
                 WHEN LENGTH(opaque)<256 THEN '1..255'
                 WHEN LENGTH(opaque)<4096 THEN '256..4095'
                 ELSE '4096+'
               END AS bucket,COUNT(*)
        FROM ctf_entries GROUP BY bucket
    """,
}
IDENTIFIERS = {"entries", "ctf_entries", "entries_trigram", "categories", "entry_categories"}

def _columns(conn: sqlite3.Connection, table: str) -> set[str]:
    return {str(row[1]) for row in conn.execute(f"PRAGMA table_info({table})")}

def _one(conn: sqlite3.Connection, query: str, columns: tuple[str, ...]) -> dict[str, int]:
    row = conn.execute(query).fetchone()
    return {key: int(value or 0) for key, value in zip(columns, row, strict=True)}

def profile(conn: sqlite3.Connection, *, dbstat: bool = True) -> dict:
    # The connector must remain read-only; avoid PRAGMA optimize, ANALYZE or VACUUM.
    conn.execute("PRAGMA query_only=ON")
    conn.execute("PRAGMA busy_timeout=5000")
    tables = {
        name for (name,) in conn.execute(
            "SELECT name FROM sqlite_master WHERE type IN ('table','view')"
        )
    }
    page_size = int(conn.execute("PRAGMA page_size").fetchone()[0])
    page_count = int(conn.execute("PRAGMA page_count").fetchone()[0])
    freelist_count = int(conn.execute("PRAGMA freelist_count").fetchone()[0])
    report: dict = {
        "format": "whereisit-ai-storage-audit-v1",
        "sqlite_version": sqlite3.sqlite_version,
        "page_size": page_size,
        "page_count": page_count,
        "allocated_bytes": page_size * page_count,
        "freelist_bytes": page_size * freelist_count,
        "user_version": int(conn.execute("PRAGMA user_version").fetchone()[0]),
    }
    entry_cols = _columns(conn, "entries") if "entries" in tables else set()
    if set(("description", "tags", "relative_path", "search_text", "name_norm")) <= entry_cols:
        report["entry_fields"] = _one(
            conn,
            SQL["entry_fields"],
            ("records", "descriptions_nonempty", "tags_nonempty",
             "description_bytes", "tag_bytes", "path_bytes", "name_bytes",
             "search_text_bytes", "normalized_name_bytes"),
        )
        report["entry_payload_distribution"] = {
            str(k): int(n) for k, n in conn.execute(SQL["entry_payload_distribution"])
        }
    elif "entries" in tables:
        report["entry_fields"] = {"records": int(
            conn.execute("SELECT COUNT(*) FROM entries").fetchone()[0]
        ), "schema": "not-v1"}
    if "ctf_entries" in tables and "opaque" in _columns(conn, "ctf_entries"):
        report["legacy_fields"] = _one(
            conn, SQL["legacy_fields"],
            ("records", "opaque_nonempty", "opaque_bytes", "max_opaque_bytes")
        )
        report["legacy_payload_distribution"] = {
            str(k): int(n) for k, n in conn.execute(SQL["legacy_payload_distribution"])
        }
    if dbstat:
        # dbstat is a SQLite build option; unavailable is not fatal.
        try:
            report["objects_bytes"] = {
                str(name): int(size)
                for name, size in conn.execute(
                    "SELECT name, SUM(pgsize) FROM dbstat "
                    "GROUP BY name ORDER BY SUM(pgsize) DESC"
                )
            }
        except sqlite3.OperationalError as e:
            report["objects_bytes_status"] = "unavailable"
            report["objects_bytes_reason"] = type(e).__name__
    # Aggregate-only metrics: never report names/paths/text, even error details.
    return report

def connect_readonly(path: Path) -> sqlite3.Connection:
    if not path.is_file():
        raise FileNotFoundError("Catalog file is not a regular file")
    # URI quoting is needed for Windows paths containing '#' or '?'.
    path_uri = quote(path.resolve().as_posix(), safe="/:")
    return sqlite3.connect(f"file:{path_uri}?mode=ro", uri=True)

def self_test() -> None:
    conn = sqlite3.connect(":memory:")
    conn.executescript("""
      CREATE TABLE entries (
        id INTEGER PRIMARY KEY, name TEXT NOT NULL,
        description TEXT NOT NULL DEFAULT '',tags TEXT NOT NULL DEFAULT '',
        relative_path TEXT NOT NULL,search_text TEXT NOT NULL,
        name_norm TEXT NOT NULL
      );
      CREATE TABLE ctf_entries (
        entry_id INTEGER PRIMARY KEY, opaque BLOB NOT NULL DEFAULT x''
      );
      INSERT INTO entries VALUES(1,'one','','','one','one','one');
      INSERT INTO entries VALUES(2,'two','note','tag','folder/two','two note tag','two');
      INSERT INTO ctf_entries VALUES(1,x'');
      INSERT INTO ctf_entries VALUES(2,x'010203');
    """)
    out = profile(conn)
    assert out["entry_fields"]["records"] == 2
    assert out["entry_fields"]["descriptions_nonempty"] == 1
    assert out["entry_fields"]["tags_nonempty"] == 1
    assert out["entry_payload_distribution"]["empty"] == 1
    assert out["legacy_fields"]["opaque_nonempty"] == 1
    assert out["legacy_fields"]["opaque_bytes"] == 3
    conn.close()

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("database", nargs="?", type=Path, help="Local SQLite file (read-only)")
    ap.add_argument("--out", type=Path, help="Optional JSON output path, not the database")
    ap.add_argument("--no-dbstat", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        self_test()
        print("storage_audit self-test: PASS")
        return 0
    if args.database is None:
        ap.error("database is required unless --self-test is set")
    try:
        with connect_readonly(args.database) as conn:
            data = profile(conn, dbstat=not args.no_dbstat)
    except (OSError, sqlite3.Error) as e:
        # Only the exception class: never echo a private path.
        print(f"storage_audit failed: {type(e).__name__}", file=sys.stderr)
        return 2
    text = json.dumps(data, ensure_ascii=False, sort_keys=True, indent=2) + "\n"
    if args.out:
        args.out.write_text(text, encoding="utf-8")
    else:
        sys.stdout.write(text)
    return 0

if __name__ == "__main__":
    sys.exit(main())
