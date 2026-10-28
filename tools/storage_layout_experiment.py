#!/usr/bin/env python3
"""Build a disposable SQLite hot/cold layout experiment from a read-only v1 catalog.

This is NOT an in-place upgrade and NOT a production database migration.
The output file contains private catalog data: do not commit or upload it.
"""
from __future__ import annotations

import argparse
import json
import sqlite3
import sys
import tempfile
import time
from pathlib import Path
from urllib.parse import quote

CORE_SCHEMA = """
CREATE TABLE entries_core(
  id INTEGER PRIMARY KEY,volume_id INTEGER NOT NULL,parent_id INTEGER,
  name TEXT NOT NULL,relative_path TEXT NOT NULL,
  kind TEXT NOT NULL,size INTEGER NOT NULL,
  modified_unix INTEGER,name_norm TEXT NOT NULL,ext TEXT NOT NULL
);
CREATE TABLE entry_payload(
  id INTEGER PRIMARY KEY REFERENCES entries_core(id) ON DELETE CASCADE,
  description TEXT NOT NULL DEFAULT '', tags TEXT NOT NULL DEFAULT ''
);
CREATE INDEX hot_tree ON entries_core(volume_id,parent_id,name,id);
CREATE INDEX hot_extension ON entries_core(ext);
CREATE INDEX hot_duplicates ON entries_core(kind,name_norm,size);
CREATE INDEX hot_name_sort ON entries_core(name COLLATE NOCASE,id);
CREATE TABLE ctf_fixed(
  entry_id INTEGER PRIMARY KEY,
  attributes INTEGER NOT NULL,modified_dos INTEGER NOT NULL,
  created_dos INTEGER NOT NULL,accessed_dos INTEGER NOT NULL
);
CREATE TABLE ctf_opaque(
  entry_id INTEGER PRIMARY KEY REFERENCES ctf_fixed(entry_id) ON DELETE CASCADE,
  bytes BLOB NOT NULL
);
CREATE VIRTUAL TABLE core_trigram USING fts5(
  name,relative_path,content='entries_core',content_rowid='id',
  tokenize='trigram',detail=none
);
CREATE VIRTUAL TABLE payload_trigram USING fts5(
  description,tags,content='entry_payload',content_rowid='id',
  tokenize='trigram',detail=none
);
"""

REQUIRED = {
    "id","volume_id","parent_id","name","relative_path","kind",
    "size","modified_unix","description","tags","name_norm","ext"
}

def readonly_uri(path: Path) -> str:
    return "file:" + quote(path.resolve().as_posix(), safe="/:") + "?mode=ro"

def columns(con: sqlite3.Connection, schema: str, table: str) -> set[str]:
    return {str(row[1]) for row in con.execute(f"PRAGMA {schema}.table_info({table})")}

def count(con: sqlite3.Connection, sql: str) -> int:
    return int(con.execute(sql).fetchone()[0])

def convert(source: Path, target: Path, *, full_verify: bool=True) -> dict:
    """Read-only source -> NEW target path; failure cleans only the NEW .building file."""
    if not source.is_file():
        raise FileNotFoundError("Source SQLite does not exist")
    if source.resolve() == target.resolve() or target.exists() or target.is_symlink():
        raise ValueError("Refusing to overwrite source or existing destination")
    if not target.parent.is_dir():
        raise ValueError("Target parent directory does not exist")
    building = target.with_name(target.name + ".building")
    if building.exists() or building.is_symlink():
        raise ValueError("A previous partial target exists; inspect and remove manually")

    t0 = time.monotonic()
    conn = None
    try:
        conn = sqlite3.connect(str(building), isolation_level=None)
        conn.execute("PRAGMA foreign_keys=ON")
        conn.execute("PRAGMA synchronous=FULL")
        conn.execute("PRAGMA journal_mode=DELETE")
        conn.execute("ATTACH DATABASE ? AS src", (readonly_uri(source),))
        missing = REQUIRED - columns(conn, "src", "entries")
        if missing:
            raise ValueError("Unsupported source entries schema")
        if count(conn, "SELECT COUNT(*) FROM src.sqlite_master WHERE type='table' AND name='volumes'") != 1:
            raise ValueError("Missing source volumes table")
        conn.executescript(CORE_SCHEMA)
        conn.execute("BEGIN IMMEDIATE")
        # Never materialize source CTF, descriptions or strings in Python.
        conn.execute(
            "INSERT INTO entries_core "
            "(id,volume_id,parent_id,name,relative_path,kind,size,modified_unix,name_norm,ext) "
            "SELECT id,volume_id,parent_id,name,relative_path,kind,size,modified_unix,name_norm,ext "
            "FROM src.entries"
        )
        conn.execute(
            "INSERT INTO entry_payload(id,description,tags) "
            "SELECT id,description,tags FROM src.entries "
            "WHERE description<>'' OR tags<>''"
        )
        for table in ("volumes","categories","entry_categories","ctf_sources","ctf_volumes"):
            if count(conn, f"SELECT COUNT(*) FROM src.sqlite_master WHERE type='table' AND name='{table}'"):
                conn.execute(f"CREATE TABLE snapshot_{table} AS SELECT * FROM src.{table}")
        has_legacy = count(conn,"SELECT COUNT(*) FROM src.sqlite_master "
                               "WHERE type='table' AND name='ctf_entries'") == 1
        if has_legacy:
            required = {"entry_id","attributes","modified_dos","created_dos","accessed_dos","opaque"}
            if not required <= columns(conn,"src","ctf_entries"):
                raise ValueError("Unsupported source CTF legacy schema")
            conn.execute(
                "INSERT INTO ctf_fixed SELECT entry_id,attributes,modified_dos,"
                "created_dos,accessed_dos FROM src.ctf_entries"
            )
            conn.execute(
                "INSERT INTO ctf_opaque SELECT entry_id,opaque "
                "FROM src.ctf_entries WHERE LENGTH(opaque)>0"
            )
        conn.execute(
            "INSERT INTO core_trigram(rowid,name,relative_path) "
            "SELECT id,name,relative_path FROM entries_core"
        )
        conn.execute(
            "INSERT INTO payload_trigram(rowid,description,tags) "
            "SELECT id,description,tags FROM entry_payload"
        )
        conn.commit()
        rows = count(conn,"SELECT COUNT(*) FROM entries_core")
        payload_rows = count(conn,"SELECT COUNT(*) FROM entry_payload")
        legacy_rows = count(conn,"SELECT COUNT(*) FROM ctf_fixed")
        opaque_rows = count(conn,"SELECT COUNT(*) FROM ctf_opaque")
        if rows != count(conn,"SELECT COUNT(*) FROM src.entries"):
            raise AssertionError("Entry count changed")
        if count(conn,"SELECT COUNT(*) FROM entry_payload WHERE description='' AND tags=''"):
            raise AssertionError("An empty payload row was stored")
        if has_legacy and legacy_rows != count(conn,"SELECT COUNT(*) FROM src.ctf_entries"):
            raise AssertionError("CTF fixed rows changed")
        mismatch = None
        legacy_mismatch = None
        if full_verify:
            # NULL-safe comparisons; test every entry, not just a sample.
            mismatch = count(conn, """
              SELECT COUNT(*) FROM src.entries e
              LEFT JOIN entries_core c ON c.id=e.id
              LEFT JOIN entry_payload p ON p.id=e.id
              WHERE c.id IS NULL OR
                e.volume_id IS NOT c.volume_id OR e.parent_id IS NOT c.parent_id OR
                e.name IS NOT c.name OR e.relative_path IS NOT c.relative_path OR
                e.kind IS NOT c.kind OR e.size IS NOT c.size OR
                e.modified_unix IS NOT c.modified_unix OR
                e.name_norm IS NOT c.name_norm OR e.ext IS NOT c.ext OR
                e.description IS NOT COALESCE(p.description,'') OR
                e.tags IS NOT COALESCE(p.tags,'')
            """)
            if mismatch:
                raise AssertionError("Hot/cold reconstruction mismatch")
            if has_legacy:
                legacy_mismatch=count(conn, """
                  SELECT COUNT(*) FROM src.ctf_entries a
                  LEFT JOIN ctf_fixed f ON f.entry_id=a.entry_id
                  LEFT JOIN ctf_opaque p ON p.entry_id=a.entry_id
                  WHERE f.entry_id IS NULL OR
                    a.attributes IS NOT f.attributes OR
                    a.modified_dos IS NOT f.modified_dos OR
                    a.created_dos IS NOT f.created_dos OR
                    a.accessed_dos IS NOT f.accessed_dos OR
                    a.opaque IS NOT COALESCE(p.bytes,x'')
                """)
                if legacy_mismatch:
                    raise AssertionError("CTF opaque payload changed")
        integrity = conn.execute("PRAGMA quick_check").fetchone()[0]
        if integrity != "ok":
            raise AssertionError("SQLite quick_check failed")
        conn.execute("DETACH DATABASE src")
        conn.close()
        conn=None
        building.replace(target)
        return {
            "status":"EXPERIMENTAL, NOT AN APP-READABLE MIGRATION",
            "source_bytes":source.stat().st_size,
            "experiment_bytes":target.stat().st_size,
            "entries":rows,
            "sparse_description_tag_rows":payload_rows,
            "ctf_fixed_rows":legacy_rows,
            "ctf_nonempty_opaque_rows":opaque_rows,
            "all_rows_verified":full_verify,
            "entry_mismatches":mismatch,
            "legacy_mismatches":legacy_mismatch,
            "quick_check":integrity,
            "seconds":round(time.monotonic()-t0,3),
        }
    finally:
        if conn is not None:
            conn.close()
        if building.exists():
            building.unlink()

def self_test() -> None:
    with tempfile.TemporaryDirectory(prefix="whereisit-storage-") as tmp:
        root = Path(tmp)
        src, dst = root/"fixture.sqlite", root/"out.sqlite"
        con = sqlite3.connect(src)
        con.executescript("""
          CREATE TABLE volumes(id INTEGER PRIMARY KEY,label TEXT);
          INSERT INTO volumes VALUES(1,'fixture');
          CREATE TABLE entries(
            id INTEGER PRIMARY KEY,volume_id INTEGER,parent_id INTEGER,
            name TEXT,relative_path TEXT,kind TEXT,size INTEGER,
            modified_unix INTEGER,description TEXT,tags TEXT,
            search_text TEXT,name_norm TEXT,ext TEXT
          );
          INSERT INTO entries VALUES
            (1,1,NULL,'Кино','Кино','directory',0,NULL,'','','кино','кино',''),
            (2,1,1,'movie.mkv','Кино/movie.mkv','file',500,NULL,'Описание','','movie','movie.mkv','mkv'),
            (3,1,1,'music.flac','Кино/music.flac','file',200,NULL,'','tag','music','music.flac','flac'),
            (4,1,1,'empty.txt','Кино/empty.txt','file',0,NULL,'','','empty','empty.txt','txt');
          CREATE TABLE ctf_entries(entry_id INTEGER PRIMARY KEY,attributes INTEGER NOT NULL,
            modified_dos INTEGER NOT NULL,created_dos INTEGER NOT NULL,
            accessed_dos INTEGER NOT NULL,opaque BLOB NOT NULL);
          INSERT INTO ctf_entries VALUES (1,16,0,0,0,x''),(2,32,1,1,1,x'010203'),
            (3,32,2,2,2,x''),(4,32,3,3,3,x'');
        """)
        con.commit()
        con.close()
        result=convert(src,dst)
        assert result["entries"]==4
        assert result["sparse_description_tag_rows"]==2
        assert result["ctf_nonempty_opaque_rows"]==1
        assert result["entry_mismatches"]==0
        assert result["legacy_mismatches"]==0
        try:
            convert(src,src)
            raise AssertionError("Source overwrite guard failed")
        except ValueError:
            pass

def main() -> int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source",type=Path)
    parser.add_argument("--target",type=Path)
    parser.add_argument("--fast",action="store_true",
                        help="Skip full value parity (not valid for release acceptance)")
    parser.add_argument("--self-test",action="store_true")
    args=parser.parse_args()
    if args.self_test:
        self_test()
        print("storage layout self-test: PASS")
        return 0
    if not args.source or not args.target:
        parser.error("--source and --target are required")
    try:
        result=convert(args.source,args.target,full_verify=not args.fast)
    except (sqlite3.Error,OSError,ValueError,AssertionError) as error:
        print("storage experiment failed: "+type(error).__name__,file=sys.stderr)
        return 2
    print(json.dumps(result,sort_keys=True,indent=2))
    return 0

if __name__ == "__main__":
    sys.exit(main())
