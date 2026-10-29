#!/usr/bin/env python3
"""Private local CTF -> current native SQLite v3 -> CTF verification.

This script NEVER uploads, logs or prints file names, paths, catalog contents,
sample titles, descriptions, or raw query rows. Run only on a trusted machine
against a freshly built, SHA-recorded local atlas CLI binary. The original
CTF is read only; all derived SQL/export bytes live in an isolated temporary
directory that is removed when this process exits normally.

A successful run is NOT acceptance of a modified CTF in the original WhereIsIt.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
try:
    import resource
except ImportError:  # Native Windows local QA
    resource = None
import subprocess
import sys
import tempfile
import time
from typing import Any


def file_hash(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(4 * 1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def measured_cli(cli: pathlib.Path, request: dict[str, Any], timeout: int) -> tuple[dict, dict]:
    start = time.perf_counter()
    process = subprocess.run(
        [str(cli), "request", json.dumps(request, separators=(",", ":"), ensure_ascii=False)],
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
        encoding="utf-8", errors="replace", timeout=timeout, check=False,
    )
    elapsed = round((time.perf_counter() - start) * 1000, 2)
    try:
        parsed = json.loads(process.stdout)
    except ValueError:
        raise RuntimeError("invalid_native_json") from None
    if not isinstance(parsed, dict):
        raise RuntimeError("invalid_native_schema")
    if process.returncode != 0 or parsed.get("ok") is not True:
        error = parsed.get("error")
        code = error.get("code", "unknown") if isinstance(error, dict) else "unknown"
        # Never propagate arbitrary error messages: they could contain paths.
        safe_code = str(code) if str(code).replace("_", "").isalnum() else "untrusted"
        raise RuntimeError(f"native_{safe_code}")
    data = parsed.get("data")
    if not isinstance(data, dict):
        raise RuntimeError("invalid_native_data")
    return data, {"elapsed_ms": elapsed, "exit_code": process.returncode}


def ids_digest(data: dict) -> tuple[int, str]:
    items = data.get("items")
    if not isinstance(items, list):
        raise RuntimeError("search_items_missing")
    h = hashlib.sha256()
    for item in items:
        identifier = item.get("id") if isinstance(item, dict) else None
        if not isinstance(identifier, int) or identifier < 1:
            raise RuntimeError("invalid_search_id")
        h.update(identifier.to_bytes(8, "little", signed=False))
    return len(items), h.hexdigest()


def inspect(source: pathlib.Path, cli: pathlib.Path, timeout: int,
            expected_sha: str | None, full_export: bool, result: dict) -> dict:
    size = source.stat().st_size
    original_sha = file_hash(source)
    if expected_sha is not None and original_sha != expected_sha:
        raise RuntimeError("unexpected_ctf_identity")
    version = subprocess.run(
        [str(cli), "--sqlite-version"], stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL, text=True, timeout=15, check=True,
    ).stdout.strip()
    if version != "3.51.3":
        raise RuntimeError("unpatched_sqlite")
    result.update({
        "format": "whereisit-local-ctf-v3-acceptance-v1",
        "status": "in_progress",
        "input_type": "operator_supplied_ctf",
        "input_provenance": "not_independently_attested",
        "source": {"sha256": original_sha, "size_bytes": size},
        "cli_sha256": file_hash(cli),
        "sqlite_runtime_version": version,
        "private_names_paths_and_rows_exported": False,
        "source_written": False,
        "test_original_whereisit_compatibility": False,
        "phases": {},
        "limitations": [
            "Original WhereIsIt modified-CTF acceptance not tested",
            "This tests one local source, not every CTF variant or catalog",
            "No guaranteed cold-cache or p95 timings",
            "Metadata duplicates are not content-hash matches",
        ],
    })
    with tempfile.TemporaryDirectory(prefix="whereisit-v3-private-") as temp:
        root = pathlib.Path(temp)
        database, exported = root / "derived-v3.sqlite", root / "exported.ctf"
        start = time.perf_counter()
        _, metric = measured_cli(
            cli, {"op": "import_ctf", "db": str(database), "path": str(source)}, timeout,
        )
        result["phases"]["import_ctf"] = metric
        stats, metric = measured_cli(cli, {"op": "stats", "db": str(database)}, timeout)
        volumes, entries = stats.get("volumes"), stats.get("entries")
        if not isinstance(volumes, int) or not isinstance(entries, int) or volumes <= 0 or entries <= 0:
            raise RuntimeError("invalid_import_counts")
        result["volumes"], result["entries"] = volumes, entries
        result["phases"]["stats"] = metric

        # Validate the database using product API before exporting anything.
        _, metric = measured_cli(cli, {"op": "validate_catalog", "db": str(database)}, timeout)
        result["phases"]["validate_catalog"] = metric

        queries = [
            "ext:mp3",
            "ext:mkv",
            "(ext:flac OR ext:mp3) AND size:>100MiB",
            "NOT ext:tmp",
        ]
        results = []
        for n, expression in enumerate(queries, 1):
            request = {"db": str(database), "query": expression, "limit": 100, "offset": 0}
            fast, fast_metric = measured_cli(cli, {"op": "search_fast", **request}, timeout)
            full, full_metric = measured_cli(cli, {"op": "search", **request}, timeout)
            count_fast, digest_fast = ids_digest(fast)
            count_full, digest_full = ids_digest(full)
            if count_fast != count_full or digest_fast != digest_full:
                raise RuntimeError("search_page_mismatch")
            total = full.get("total")
            if not isinstance(total, int) or total < count_full:
                raise RuntimeError("invalid_search_total")
            results.append({
                "query_number": n,
                "query_text_omitted": True,
                "total_count": total,
                "page_length": count_full,
                "page_ids_sha256": digest_full,
                "fast_ms": fast_metric["elapsed_ms"],
                "full_count_ms": full_metric["elapsed_ms"],
                "page_and_count_agree": True,
            })
        result["queries"] = results

        _, metric = measured_cli(cli, {"op": "export_ctf", "db": str(database),
                                        "path": str(exported)}, timeout)
        encoded_sha = file_hash(exported)
        exact = encoded_sha == original_sha and exported.stat().st_size == size
        result["phases"]["export_ctf"] = metric
        result["ctf_roundtrip"] = {
            "byte_identical": exact, "output_sha256": encoded_sha,
            "output_bytes": exported.stat().st_size,
        }
        if not exact:
            raise RuntimeError("ctf_roundtrip_mismatch")

        if full_export:
            artifacts = []
            for ext, op in (("csv", "export_csv"), ("parquet", "export_parquet")):
                output = root / ("derived." + ext)
                _, metric = measured_cli(
                    cli, {"op": op, "db": str(database), "path": str(output)}, timeout,
                )
                artifacts.append({"format": ext, "bytes": output.stat().st_size,
                                  "sha256": file_hash(output), **metric})
            result["derived_export_artifacts"] = artifacts
        result["phases"]["total_import_to_roundtrip"] = {
            "elapsed_ms": round((time.perf_counter() - start) * 1000, 2)
        }
        result["database_bytes_before_close"] = database.stat().st_size
        result["derived_temporary_files_removed_after_run"] = True
    return result


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--cli", required=True, type=pathlib.Path)
    ap.add_argument("--output", required=True, type=pathlib.Path,
                    help="A NEW local directory for aggregate-only checkpoint")
    ap.add_argument("--expected-sha256", help="Optional verified source SHA-256")
    ap.add_argument("--full-export", action="store_true",
                    help="Also test full temporary CSV/Parquet export (larger disk I/O)")
    ap.add_argument("--timeout-seconds", type=int, default=1500,
                    help="Per-phase timeout; large 3M-entry CTF imports can exceed 8 minutes")
    ap.add_argument("source", type=pathlib.Path)
    args = ap.parse_args()
    if args.timeout_seconds < 10:
        ap.error("Timeout must be >= 10 seconds")
    cli, source = args.cli.resolve(strict=True), args.source.resolve(strict=True)
    if not cli.is_file() or not source.is_file():
        ap.error("CLI and source must be regular files")
    output = args.output.resolve()
    if output.exists() or output.is_symlink() or output == source.parent:
        ap.error("Checkpoint must be a NEW directory separate from the source")
    output.mkdir(parents=True, exist_ok=False)
    started = time.perf_counter()
    report = {"status": "in_progress",
              "source": {"sha256": file_hash(source), "size_bytes": source.stat().st_size}}
    try:
        inspect(source, cli, args.timeout_seconds, args.expected_sha256,
                args.full_export, report)
        report["status"] = "passed_available_checks"
    except Exception as exc:
        # Exception text may include a private path; store only an error category
        # or a known, code-like phase failure without file content.
        error = str(exc)
        safe_error = error if error.replace("_", "").isalnum() else type(exc).__name__
        report["status"] = "failed"
        report["error_code"] = safe_error
    finally:
        after = file_hash(source)
        before = report.get("source", {}).get("sha256") if isinstance(report, dict) else None
        report["source_sha256_after"] = after
        report["source_unchanged"] = bool(before and before == after)
        report["elapsed_seconds"] = round(time.perf_counter() - started, 3)
        if not report["source_unchanged"]:
            report["status"] = "failed"
            report["error_code"] = "source_changed"
        if sys.platform == "linux" and resource is not None:
            report["max_child_rss_kib"] = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
        report["source_original_name_and_path_omitted"] = True
        (output / "V3_ACCEPTANCE.json").write_text(
            json.dumps(report, ensure_ascii=False, sort_keys=True, indent=2) + "\n",
            encoding="utf-8",
        )
        payload = (output / "V3_ACCEPTANCE.json").read_bytes()
        (output / "SHA256SUMS.txt").write_text(
            hashlib.sha256(payload).hexdigest() + "  V3_ACCEPTANCE.json\n",
            encoding="utf-8",
        )
        print(json.dumps({
            "status": report["status"],
            "source_unchanged": report["source_unchanged"],
            "entries": report.get("entries"),
            "volumes": report.get("volumes"),
            "ctf_byte_identical": report.get("ctf_roundtrip", {}).get("byte_identical"),
            "error_code": report.get("error_code"),
        }, ensure_ascii=False))
    return 0 if report["status"] == "passed_available_checks" else 1


if __name__ == "__main__":
    sys.exit(main())
