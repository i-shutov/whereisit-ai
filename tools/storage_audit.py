#!/usr/bin/env python3
"""Read-only aggregate catalog audit. Never prints private record content.

Use a backup/copy of the catalog. --out creates a NEW file exclusively; an
existing file, symlink, database or SQLite sidecar is never overwritten.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import sqlite3
import sys
import tempfile
from pathlib import Path


def _columns(conn: sqlite3.Connection, table: str) -> set[str]:
    if table not in {"entries", "ctf_entries", "ctf_entry_payload"}:
        raise ValueError("Unapproved table")
    return {str(row[1]) for row in conn.execute(f"PRAGMA table_info({table})")}


def _row(conn: sqlite3.Connection, sql: str) -> dict[str, int]:
    cur = conn.execute(sql)
    return {str(col[0]): int(value or 0)
            for col, value in zip(cur.description, cur.fetchone(), strict=True)}


def profile(conn: sqlite3.Connection, *, dbstat: bool = True) -> dict:
    conn.execute("PRAGMA query_only=ON")
    conn.execute("PRAGMA busy_timeout=5000")
    owns_snapshot = not conn.in_transaction
    if owns_snapshot:
        conn.execute("BEGIN")
    try:
        tables = {name for (name,) in conn.execute(
            "SELECT name FROM sqlite_master WHERE type IN ('table','view')")}
        size = int(conn.execute("PRAGMA page_size").fetchone()[0])
        pages = int(conn.execute("PRAGMA page_count").fetchone()[0])
        free = int(conn.execute("PRAGMA freelist_count").fetchone()[0])
        result = {
            "format": "whereisit-ai-storage-audit-v2",
            "sqlite_version": sqlite3.sqlite_version,
            "user_version": int(conn.execute("PRAGMA user_version").fetchone()[0]),
            "page_size": size, "page_count": pages,
            "allocated_bytes": size * pages, "freelist_bytes": size * free,
            "consistent_read_snapshot": True,
        }
        cols = _columns(conn, "entries") if "entries" in tables else set()
        required = {"name", "description", "tags", "relative_path", "search_text", "name_norm"}
        if required <= cols:
            result["entry_fields"] = _row(conn, """
                SELECT COUNT(*) AS records,
                COUNT(NULLIF(description,'')) AS descriptions_nonempty,
                COUNT(NULLIF(tags,'')) AS tags_nonempty,
                SUM(LENGTH(CAST(description AS BLOB))) AS description_bytes,
                SUM(LENGTH(CAST(tags AS BLOB))) AS tag_bytes,
                SUM(LENGTH(CAST(relative_path AS BLOB))) AS path_bytes,
                SUM(LENGTH(CAST(name AS BLOB))) AS name_bytes,
                SUM(LENGTH(CAST(search_text AS BLOB))) AS search_text_bytes,
                SUM(LENGTH(CAST(name_norm AS BLOB))) AS normalized_name_bytes
                FROM entries
            """)
            result["entry_payload_distribution"] = dict(conn.execute("""
                SELECT CASE
                  WHEN description<>'' AND tags<>'' THEN 'description_and_tags'
                  WHEN description<>'' THEN 'description_only'
                  WHEN tags<>'' THEN 'tags_only' ELSE 'empty' END, COUNT(*)
                FROM entries GROUP BY 1
            """))
        elif "entries" in tables:
            result["entry_fields"] = _row(conn, "SELECT COUNT(*) AS records FROM entries")
            result["entry_fields"]["schema"] = "unknown"
        if "ctf_entries" in tables:
            dense = "opaque" in _columns(conn, "ctf_entries")
            payload_table = "ctf_entries" if dense else "ctf_entry_payload"
            result["legacy_layout"] = "dense-v1" if dense else "sparse-v2"
            result["legacy_records"] = int(conn.execute("SELECT COUNT(*) FROM ctf_entries").fetchone()[0])
            if payload_table in tables:
                result["legacy_fields"] = _row(conn, f"""
                    SELECT COUNT(*) AS stored_rows,
                    SUM(CASE WHEN LENGTH(opaque)>0 THEN 1 ELSE 0 END) AS opaque_nonempty,
                    SUM(LENGTH(opaque)) AS opaque_bytes,
                    MAX(LENGTH(opaque)) AS max_opaque_bytes FROM {payload_table}
                """)
                result["legacy_payload_distribution"] = dict(conn.execute(f"""
                    SELECT CASE WHEN LENGTH(opaque)=0 THEN 'empty'
                      WHEN LENGTH(opaque)<256 THEN '1..255'
                      WHEN LENGTH(opaque)<4096 THEN '256..4095' ELSE '4096+' END,
                      COUNT(*) FROM {payload_table} GROUP BY 1
                """))
        if dbstat:
            try:
                result["objects_bytes"] = dict(conn.execute(
                    "SELECT name,SUM(pgsize) FROM dbstat GROUP BY name ORDER BY SUM(pgsize) DESC"))
            except sqlite3.OperationalError:
                result["objects_bytes_status"] = "dbstat unavailable"
        return result
    finally:
        if owns_snapshot:
            conn.rollback()


def connect_readonly(path: Path) -> sqlite3.Connection:
    if not path.is_file():
        raise FileNotFoundError("Catalog is not a regular file")
    conn = sqlite3.connect(path.resolve().as_uri() + "?mode=ro", uri=True)
    conn.create_function("unicode_lower", 1, lambda value: value.lower(), deterministic=True)
    return conn


def write_report_new(path: Path, text: str, source: Path) -> None:
    # Reject the source AND all of its SQLite sidecars, including nonexistent ones.
    source_path = source.resolve()
    reserved = {source_path, *(Path(str(source_path) + suffix) for suffix in ("-wal", "-shm", "-journal"))}
    if path.resolve() in reserved or path.is_symlink():
        raise ValueError("Report cannot target the catalog or its sidecars")
    # O_EXCL also rejects hard links, existing unrelated files and dangling symlinks.
    fd = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    try:
        with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as out:
            out.write(text)
            out.flush()
            os.fsync(out.fileno())
    except BaseException:
        # We exclusively created this file; never touch the source database.
        path.unlink(missing_ok=True)
        raise


def self_test() -> None:
    with tempfile.TemporaryDirectory() as temp:
        root = Path(temp)
        source = root / "fixture #?.sqlite"
        con = sqlite3.connect(source)
        con.executescript("""
          CREATE TABLE entries(id INTEGER PRIMARY KEY,name TEXT,description TEXT,
            tags TEXT,relative_path TEXT,search_text TEXT,name_norm TEXT);
          INSERT INTO entries VALUES(1,'one','','','one','one','one'),
            (2,'two','note','tag','folder/two','two note tag','two');
          CREATE TABLE ctf_entries(entry_id INTEGER PRIMARY KEY,opaque BLOB);
          INSERT INTO ctf_entries VALUES(1,x''),(2,x'010203');
        """)
        con.close()
        original = hashlib.sha256(source.read_bytes()).digest()
        for target in (source, Path(str(source) + "-wal"), Path(str(source) + "-shm")):
            try:
                write_report_new(target, "invalid", source)
                raise AssertionError("Unsafe report destination accepted")
            except (ValueError, FileExistsError):
                pass
        hardlink = root / "hardlink.json"
        os.link(source, hardlink)
        try:
            write_report_new(hardlink, "invalid", source)
            raise AssertionError("Hard link overwritten")
        except FileExistsError:
            pass
        with connect_readonly(source) as conn:
            out = profile(conn)
        assert out["entry_fields"]["records"] == 2
        assert out["entry_fields"]["descriptions_nonempty"] == 1
        assert out["legacy_fields"]["opaque_bytes"] == 3
        assert hashlib.sha256(source.read_bytes()).digest() == original
        report = root / "report.json"
        write_report_new(report, json.dumps(out), source)
        try:
            write_report_new(report, "overwrite", source)
            raise AssertionError("Existing report overwritten")
        except FileExistsError:
            pass
        con = sqlite3.connect(source)
        con.executescript("""
          DROP TABLE ctf_entries;
          CREATE TABLE ctf_entries(entry_id INTEGER PRIMARY KEY);
          INSERT INTO ctf_entries VALUES(1),(2);
          CREATE TABLE ctf_entry_payload(entry_id INTEGER PRIMARY KEY,opaque BLOB);
          INSERT INTO ctf_entry_payload VALUES(2,x'010203');
          PRAGMA user_version=2;
        """)
        con.close()
        with connect_readonly(source) as conn:
            out = profile(conn)
        assert out["legacy_layout"] == "sparse-v2"
        assert out["legacy_records"] == 2
        assert out["legacy_fields"]["stored_rows"] == 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("database", nargs="?", type=Path)
    ap.add_argument("--out", type=Path)
    ap.add_argument("--no-dbstat", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        self_test()
        print("storage audit self-test: PASS (v1/v2, read-only, output guards)")
        return 0
    if args.database is None:
        ap.error("database required unless --self-test is set")
    try:
        conn = connect_readonly(args.database)
        try:
            data = profile(conn, dbstat=not args.no_dbstat)
        finally:
            conn.close()
        text = json.dumps(data, ensure_ascii=False, sort_keys=True, indent=2) + "\n"
        if args.out:
            write_report_new(args.out, text, args.database)
        else:
            sys.stdout.write(text)
    except (OSError, ValueError, sqlite3.Error) as error:
        print("storage audit failed: " + type(error).__name__, file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
