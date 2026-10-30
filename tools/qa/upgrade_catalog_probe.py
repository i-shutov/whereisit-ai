#!/usr/bin/env python3
"""Cross-platform RC.1 -> RC.2 catalog upgrade QA; GENERATED FIXTURES ONLY.

Modes:
  prepare: initialize catalog with the installed RC.1 binary, scan generated
           files, create a Unicode category and save aggregate expectations.
  verify:  test the same external SQLite with installed RC.2; ensure the new
           installer did not alter existing database/catalog files.

No real user CTF or personal paths are accepted or uploaded. Runs only inside
a designated ephemeral CI root. Results are short nonprivate JSON.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

RC1 = "1.0.0-rc.1"
RC2 = "1.0.0-rc.2"
QUERY = "(ext:mp3 OR ext:mkv) AND size:>0B"
CATEGORY = "QA-Upgrade-Ω-Категория"
CTF_PLACEHOLDER = b"GENERATED INVALID CTF PLACEHOLDER; PROVE THAT INSTALLERS PRESERVE EXTERNAL FILES\n"
FORMAT = "whereisit-native-upgrade-smoke-v1"


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
        # Do not propagate native error text; it can contain local paths.
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
            [(int(x["id"]), str(x["name"]), int(x["size"])) for x in result["items"]]
        ),
        "volume_ids": sorted(int(x["id"]) for x in volumes),
    }


def verify_external(root: Path, expected: dict) -> None:
    for name, digest in expected["source_files"].items():
        p = root / name
        if not p.is_file() or sha256(p) != digest:
            raise RuntimeError("synthetic_source_changed")
    external = root / "external-user-ctf-placeholder.CTF"
    if not external.is_file() or sha256(external) != expected["placeholder_ctf_sha256"]:
        raise RuntimeError("external_ctf_modified_by_upgrade")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["prepare", "verify"])
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--db", type=Path, required=True)
    parser.add_argument("--fixture-root", type=Path, required=True)
    parser.add_argument("--snapshot", type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get("CI", "").lower() != "true":
        parser.error("This destructive installer QA is restricted to ephemeral CI")
    cli, db = args.cli.resolve(strict=True), args.db.absolute()
    root, snapshot = args.fixture_root.absolute(), args.snapshot.absolute()
    expected_version = RC1 if args.mode == "prepare" else RC2
    actual_version = cli_version(cli)
    if actual_version != f"WhereIsIt AI {expected_version}":
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
        baseline = observed(cli, db)
        verify_external(root, signatures)
        snapshot.parent.mkdir(parents=True, exist_ok=True)
        snapshot.write_text(
            json.dumps({
                "format": FORMAT,
                "source_release": RC1,
                "target_release": RC2,
                "fixture_signatures": signatures,
                "baseline": baseline,
                "db_sha256_after_prepare": sha256(db),
            }, sort_keys=True, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
        print(json.dumps({
            "phase": "rc1_prepare",
            "status": "pass",
            "entry_count": baseline["entries"],
            "categories": len(baseline["category_names"]),
            "synthetic_only": True,
        }))
    else:
        checkpoint = json.loads(snapshot.read_text(encoding="utf-8"))
        if checkpoint["format"] != FORMAT or checkpoint["source_release"] != RC1:
            raise RuntimeError("wrong_checkpoint")
        if not db.is_file():
            raise RuntimeError("catalog_missing_after_upgrade")
        verify_external(root, checkpoint["fixture_signatures"])
        check = call_cli(cli, {"op": "validate_catalog", "db": str(db)})
        if not isinstance(check, dict) or check.get("quick_check") != "ok":
            raise RuntimeError("sqlite_integrity_check_failed")
        after = observed(cli, db)
        if checkpoint["baseline"] != after:
            raise RuntimeError("catalog_category_or_search_data_changed")
        print(json.dumps({
            "phase": "rc2_verify", "status": "pass",
            "entry_count": after["entries"],
            "categories": len(after["category_names"]),
            "sqlite_quick_check": "ok",
            "external_ctf_unchanged": True,
            "synthetic_only": True,
        }))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (AssertionError, OSError, RuntimeError, KeyError, ValueError, TimeoutError) as exc:
        # Error value is a constant or type name, never arbitrary native stderr.
        msg = str(exc)
        safe = msg if msg.replace("_", "").isalnum() else type(exc).__name__
        print(json.dumps({"status": "fail", "error_code": safe, "synthetic_only": True}))
        sys.exit(1)
