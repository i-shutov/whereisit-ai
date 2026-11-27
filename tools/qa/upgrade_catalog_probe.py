#!/usr/bin/env python3
"""Cross-platform installed-release catalog upgrade QA; GENERATED FIXTURES ONLY.

Modes:
  prepare: initialize catalog with the installed source-release binary, scan
           generated files, create a Unicode category and save expectations.
  verify:  test the same external SQLite with the installed target release;
           ensure the new installer did not alter existing catalog files.

No real user CTF or personal paths are accepted or uploaded. Runs only inside
an ephemeral CI root. Results are short nonprivate JSON.
"""
from __future__ import annotations

import argparse
from contextlib import closing
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys

RC1 = "1.0.0-rc.1"
RC2 = "1.0.0-rc.2"
RC3 = "1.0.0-rc.3"
RC4 = "1.0.0-rc.4"
STABLE = "1.0.0"
NEXT_RC1 = "1.1.0-rc.1"
NEXT_STABLE = "1.1.0"
HOTFIX_STABLE = "1.1.1"
MEMORY_FIX_STABLE = "1.1.2"
QUERY = "(ext:mp3 OR ext:mkv) AND size:>0B"
CATEGORY = "QA-Upgrade-Ω-Категория"
ENTRY_NOTE = "Synthetic upgrade note — 東京"
ENTRY_TAGS = "qa,stable-upgrade"
CTF_PLACEHOLDER = b"GENERATED INVALID CTF PLACEHOLDER; PROVE THAT INSTALLERS PRESERVE EXTERNAL FILES\n"
FORMAT = "whereisit-native-upgrade-smoke-v1"
SUPPORTED_PAIRS = {
    (RC1, RC2), (RC2, RC3), (RC3, RC4), (RC4, STABLE),
    (STABLE, NEXT_RC1), (STABLE, NEXT_STABLE), (NEXT_STABLE, HOTFIX_STABLE),
    (HOTFIX_STABLE, MEMORY_FIX_STABLE),
}
USER_STORE_PAIRS = {(NEXT_STABLE, HOTFIX_STABLE), (HOTFIX_STABLE, MEMORY_FIX_STABLE)}


def expected_cli_version(mode: str, source: str, target: str) -> str:
    """Do not infer a predecessor from the working tree's current VERSION."""
    if mode not in {"prepare", "verify"} or (source, target) not in SUPPORTED_PAIRS:
        raise RuntimeError("unsupported_upgrade_version_pair")
    return "WhereIsIt AI " + (source if mode == "prepare" else target)


def validate_checkpoint_versions(checkpoint: dict, source: str, target: str) -> None:
    if (checkpoint.get("format") != FORMAT
            or checkpoint.get("source_release") != source
            or checkpoint.get("target_release") != target):
        raise RuntimeError("wrong_checkpoint")


def validate_user_store_pair(source: str, target: str, user_store: Path | None) -> None:
    """Recent stable upgrades exercise the application's real preferences sidecar."""
    if (source, target) in USER_STORE_PAIRS:
        if user_store is None:
            code = ("user_store_required_for_hotfix" if (source, target) ==
                    (NEXT_STABLE, HOTFIX_STABLE) else "user_store_required_for_memory_fix")
            raise RuntimeError(code)
    elif user_store is not None:
        raise RuntimeError("unexpected_user_store_for_upgrade_pair")


def preferences_payload(db: Path, store: Path, volume_id: int, *, note: str =
                        "Synthetic shelf preferences — 1.1.0 to 1.1.1") -> dict:
    return {
        "op": "disk_preferences_set", "db": str(db), "user_store": str(store),
        "platform": "windows", "volume_id": volume_id,
        "display_name": "Synthetic Upgrade Disk", "shelf_label": "QA-HOTFIX-111",
        "model": "Synthetic Model", "filesystem": "NTFS", "capacity_bytes": 123456,
        "note": note,
        "expected_revision": 0, "expected_shelf_label": "", "confirmed": True,
    }


def preferences_snapshot(cli: Path, db: Path, store: Path, volume_id: int,
                         *, create: bool, note: str =
                         "Synthetic shelf preferences — 1.1.0 to 1.1.1") -> dict:
    if create:
        saved = call_cli(cli, preferences_payload(db, store, volume_id, note=note))
        if (not isinstance(saved, dict) or saved.get("saved") is not True
                or saved.get("volume_id") != volume_id or saved.get("revision") != 1):
            raise RuntimeError("synthetic_user_store_save_failed")
    result = call_cli(cli, {
        "op": "disk_preferences_get", "db": str(db), "user_store": str(store),
        "platform": "windows", "volume_id": volume_id,
    })
    expected = {
        "volume_id": volume_id, "display_name": "Synthetic Upgrade Disk",
        "shelf_label": "QA-HOTFIX-111", "model": "Synthetic Model",
        "filesystem": "NTFS", "capacity_bytes": 123456,
        "note": note, "revision": 1,
    }
    if result != expected:
        raise RuntimeError("synthetic_user_store_preferences_mismatch")
    return expected


def verify_user_store_checkpoint(cli: Path, db: Path, store: Path,
                                 checkpoint: dict, *, note: str =
                                 "Synthetic shelf preferences — 1.1.0 to 1.1.1") -> bool:
    if str(store) != checkpoint["path"]:
        raise RuntimeError("user_store_path_changed")
    digest = checkpoint["sha256_after_prepare"]
    if not store.is_file() or sha256(store) != digest:
        raise RuntimeError("user_store_bytes_changed_by_upgrade")
    volume_id = int(checkpoint["preferences"]["volume_id"])
    preferences = preferences_snapshot(cli, db, store, volume_id, create=False, note=note)
    if sha256(store) != digest:
        raise RuntimeError("user_store_bytes_changed_by_read")
    if preferences != checkpoint["preferences"]:
        raise RuntimeError("user_store_semantics_changed_by_upgrade")
    return True


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def call_cli(cli: Path, payload: dict) -> object:
    request = json.dumps(payload, ensure_ascii=False, separators=(",", ":"))
    outcome = subprocess.run(
        [str(cli), "request", request],
        text=True, encoding="utf-8", errors="replace",
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        check=False, timeout=90,
    )
    if outcome.returncode != 0:
        raise RuntimeError("native_request_failed")
    try:
        envelope = json.loads(outcome.stdout)
    except json.JSONDecodeError:
        raise RuntimeError("invalid_native_json") from None
    if not isinstance(envelope, dict) or envelope.get("ok") is not True:
        raise RuntimeError("native_request_not_ok")
    return envelope.get("data")


def cli_version(cli: Path) -> str:
    proc = subprocess.run(
        [str(cli), "--version"], text=True, encoding="utf-8",
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        check=True, timeout=15,
    )
    return proc.stdout.strip()


def fixture_files(root: Path) -> dict:
    if root.exists():
        raise RuntimeError("fixture_root_already_exists")
    root.mkdir(parents=False)
    files = {
        "library/Музыка/track-a.mp3": b"Synthetic test-only audio marker #1\n",
        "library/Музыка/track-b.flac": b"Synthetic test-only audio marker #2\n",
        "library/Movies/movie-a.mkv": b"Synthetic test-only video marker #3\n",
    }
    for name, value in files.items():
        p = root / name
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(value)
    external = root / "external-user-ctf-placeholder.CTF"
    external.write_bytes(CTF_PLACEHOLDER)
    return {
        "source_files": {name: sha256(root / name) for name in sorted(files)},
        "placeholder_ctf_sha256": sha256(external),
    }


def observed(cli: Path, db: Path) -> dict:
    stats = call_cli(cli, {"op": "stats", "db": str(db)})
    if not isinstance(stats, dict):
        raise RuntimeError("invalid_stats_type")
    volumes = call_cli(cli, {"op": "volumes", "db": str(db)})
    categories = call_cli(cli, {"op": "categories", "db": str(db)})
    result = call_cli(cli, {
        "op": "search", "db": str(db), "query": QUERY,
        "offset": 0, "limit": 20,
    })
    if not isinstance(volumes, list) or not isinstance(categories, list):
        raise RuntimeError("invalid_api_lists")
    if not isinstance(result, dict) or not isinstance(result.get("items"), list):
        raise RuntimeError("invalid_search_page")
    entries = int(stats["entries"])
    assert entries >= 3 and len(volumes) == 1, "Synthetic scan missing"
    assert int(result["total"]) == 2, "Two extension matches required"
    assert any(x["name"] == CATEGORY for x in categories), "Unicode category missing"
    return {
        "volumes": int(stats["volumes"]),
        "entries": entries,
        "bytes": int(stats["total_bytes"]),
        "category_names": sorted(x["name"] for x in categories),
        "category_ids": sorted(int(x["id"]) for x in categories),
        "search_total": int(result["total"]),
        "search_items": sorted(
            [[int(x["id"]), str(x["name"]), int(x["size"])] for x in result["items"]]
        ),
        "volume_ids": sorted(int(x["id"]) for x in volumes),
        "entry_notes": sorted(
            [[int(x["id"]), str(x["description"]), str(x["tags"])]
             for x in result["items"]]
        ),
    }


def verify_external(root: Path, expected: dict) -> None:
    for name, digest in expected["source_files"].items():
        p = root / name
        if not p.is_file() or sha256(p) != digest:
            raise RuntimeError("synthetic_source_changed")
    external = root / "external-user-ctf-placeholder.CTF"
    if not external.is_file() or sha256(external) != expected["placeholder_ctf_sha256"]:
        raise RuntimeError("external_ctf_modified_by_upgrade")


def seed_fixture_annotation(db: Path) -> None:
    """Seed only the disposable fixture, including its metadata-index triggers."""
    with closing(sqlite3.connect(db)) as connection, connection:
        # Catalog::open registers this function for metadata/FTS triggers.
        # The fixture note/tags use ASCII and uncased Japanese characters.
        connection.create_function("unicode_lower", 1, str.lower, deterministic=True)
        target = "Музыка/track-a.mp3"
        if connection.execute(
                "SELECT COUNT(*) FROM entries WHERE relative_path=?", (target,)
        ).fetchone()[0] != 1:
            raise RuntimeError("synthetic_note_row_missing")
        connection.execute(
            "UPDATE entries SET description=?, tags=? WHERE relative_path=?",
            (ENTRY_NOTE, ENTRY_TAGS, target),
        )


def require_fixture_annotation(baseline: dict) -> None:
    if not any(note == ENTRY_NOTE and tags == ENTRY_TAGS
               for entry_id, note, tags in baseline["entry_notes"]):
        raise RuntimeError("synthetic_note_not_visible")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["prepare", "verify"])
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--db", type=Path, required=True)
    parser.add_argument("--fixture-root", type=Path, required=True)
    parser.add_argument("--snapshot", type=Path, required=True)
    parser.add_argument("--user-store", type=Path)
    parser.add_argument("--source-release", default=RC1)
    parser.add_argument("--target-release", default=RC2)
    args = parser.parse_args()
    if os.environ.get("CI", "").lower() != "true":
        parser.error("This destructive installer QA is restricted to ephemeral CI")
    cli, db = args.cli.resolve(strict=True), args.db.absolute()
    root, snapshot = args.fixture_root.absolute(), args.snapshot.absolute()
    expected_version = expected_cli_version(args.mode, args.source_release, args.target_release)
    validate_user_store_pair(args.source_release, args.target_release, args.user_store)
    user_store = args.user_store.absolute() if args.user_store is not None else None
    actual_version = cli_version(cli)
    if actual_version != expected_version:
        raise RuntimeError("unexpected_installed_cli_version")
    if args.mode == "prepare":
        if db.exists() or snapshot.exists():
            raise RuntimeError("refuse_to_overwrite_user_catalog_or_checkpoint")
        signatures = fixture_files(root)
        call_cli(cli, {"op": "init", "db": str(db)})
        volume = call_cli(cli, {
            "op": "scan", "db": str(db), "root": str(root / "library"),
            "label": "Synthetic offline upgrade media",
        })
        if not isinstance(volume, dict) or int(volume.get("entry_count", 0)) < 3:
            raise RuntimeError("synthetic_scan_incomplete")
        call_cli(cli, {"op": "create_category", "db": str(db), "name": CATEGORY})
        seed_fixture_annotation(db)
        user_store_state = None
        if user_store is not None:
            if user_store.exists():
                raise RuntimeError("refuse_to_overwrite_user_store")
            volumes = call_cli(cli, {"op": "volumes", "db": str(db)})
            if not isinstance(volumes, list) or len(volumes) != 1:
                raise RuntimeError("synthetic_user_store_volume_missing")
            user_store_state = preferences_snapshot(
                cli, db, user_store, int(volumes[0]["id"]), create=True,
                note=f"Synthetic shelf preferences — {args.source_release} to {args.target_release}",
            )
        baseline = observed(cli, db)
        require_fixture_annotation(baseline)
        verify_external(root, signatures)
        snapshot.parent.mkdir(parents=True, exist_ok=True)
        snapshot.write_text(
            json.dumps({
                "format": FORMAT,
                "source_release": args.source_release,
                "target_release": args.target_release,
                "fixture_signatures": signatures,
                "baseline": baseline,
                "db_sha256_after_prepare": sha256(db),
                "user_store": ({
                    "path": str(user_store),
                    "sha256_after_prepare": sha256(user_store),
                    "preferences": user_store_state,
                } if user_store is not None else None),
            }, sort_keys=True, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
        print(json.dumps({
            "phase": "source_prepare", "source_release": args.source_release,
            "status": "pass",
            "entry_count": baseline["entries"],
            "categories": len(baseline["category_names"]),
            "synthetic_only": True,
        }))
    else:
        checkpoint = json.loads(snapshot.read_text(encoding="utf-8"))
        validate_checkpoint_versions(checkpoint, args.source_release, args.target_release)
        if not db.is_file():
            raise RuntimeError("catalog_missing_after_upgrade")
        verify_external(root, checkpoint["fixture_signatures"])
        check = call_cli(cli, {"op": "validate_catalog", "db": str(db)})
        if not isinstance(check, dict) or check.get("quick_check") != "ok":
            raise RuntimeError("sqlite_integrity_check_failed")
        after = observed(cli, db)
        if checkpoint["baseline"] != after:
            raise RuntimeError("catalog_category_or_search_data_changed")
        user_store_checkpoint = checkpoint.get("user_store")
        if user_store_checkpoint is not None:
            if user_store is None:
                raise RuntimeError("user_store_path_changed")
            note = f"Synthetic shelf preferences — {args.source_release} to {args.target_release}"
            verify_user_store_checkpoint(cli, db, user_store, user_store_checkpoint, note=note)
        print(json.dumps({
            "phase": "target_verify", "target_release": args.target_release, "status": "pass",
            "entry_count": after["entries"],
            "categories": len(after["category_names"]),
            "sqlite_quick_check": "ok",
            "external_ctf_unchanged": True,
            "user_store_preserved": user_store_checkpoint is not None,
            "synthetic_only": True,
        }))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, RuntimeError, KeyError, ValueError, TimeoutError) as exc:
        msg = str(exc)
        safe = msg if msg.replace("_", "").isalnum() else type(exc).__name__
        print(json.dumps({"status": "fail", "error_code": safe, "synthetic_only": True}))
        sys.exit(1)
