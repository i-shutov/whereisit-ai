#!/usr/bin/env python3
"""Developer-only aggregate benchmark for an existing Atlas catalog (Python 3.10+)."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import queue
import statistics
import subprocess
import sys
import tempfile
import threading
import time

SEARCHES = (
    ("all_first_page", ""),
    ("cyrillic_substring", "name:концерт"),
    ("latin_substring", "name:matrix"),
    ("movie_expression", "(ext:mkv OR ext:mp4 OR ext:avi) AND size:>1GiB AND NOT name:sample"),
    ("music_expression", "ext:mp3 OR ext:flac OR ext:m4a"),
    ("path_and_music", "path:music AND (ext:flac OR ext:mp3)"),
    ("glob_mkv", "name:*.mkv"),
)
DUPLICATES = (
    ("all_files", ""),
    ("movies", "ext:mkv OR ext:mp4 OR ext:avi"),
    ("music", "ext:mp3 OR ext:flac OR ext:m4a"),
)


class Service:
    def __init__(self, cli, db, timeout):
        self.db, self.timeout = str(db), timeout
        self.stderr = tempfile.TemporaryFile(mode="w+", encoding="utf-8")
        self.process = subprocess.Popen(
            [str(cli), "serve"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=self.stderr, text=True, encoding="utf-8", bufsize=1,
        )
        self.lines = queue.Queue()
        def read_stdout():
            try:
                for line in self.process.stdout:
                    self.lines.put(line)
            finally:
                self.lines.put(None)
        threading.Thread(target=read_stdout, daemon=True).start()

    def call(self, operation, **fields):
        start = time.perf_counter()
        request = {"op": operation, "db": self.db, **fields}
        self.process.stdin.write(json.dumps(request, ensure_ascii=False) + "\n")
        self.process.stdin.flush()
        try:
            line = self.lines.get(timeout=self.timeout)
        except queue.Empty as error:
            raise RuntimeError("Service request timed out: " + operation) from error
        if line is None:
            raise RuntimeError("Service exited before responding: " + operation)
        response = json.loads(line)
        if response.get("ok") is not True:
            code = response.get("error", {}).get("code", "unknown")
            raise RuntimeError("Service rejected " + operation + " (code: " + str(code) + ")")
        return response["data"], round((time.perf_counter() - start) * 1000, 3)

    def close(self):
        try:
            self.process.stdin.close()
            self.process.wait(timeout=5)
        except (BrokenPipeError, subprocess.TimeoutExpired):
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
        finally:
            self.stderr.close()


def measure(service, label, query, operation="search_fast"):
    options = {"query": query, "limit": 250, "offset": 0}
    if operation == "duplicates":
        options["mode"] = "name_size"
    first, elapsed = service.call(operation, **options)
    # IDs are compared internally to detect changing pages, never included in reports.
    first_ids = [row["id"] for row in first["items"]]
    samples = []
    for _ in range(5):
        page, duration = service.call(operation, **options)
        if [row["id"] for row in page["items"]] != first_ids:
            raise RuntimeError("Page changed during measurement; stop other catalog edits and retry")
        samples.append(duration)
    result = {
        "label": label, "query": query, "page_size": 250,
        "returned": len(first_ids), "first_ms": elapsed, "warm_samples": 5,
        "warm_p50_ms": round(statistics.median(samples), 3),
        "warm_max_ms": max(samples), "has_more": first.get("has_more"),
    }
    if "total" in first:
        result["total_matches"] = first["total"]
    if operation == "duplicates":
        result["verified"] = first.get("verified", False)
        result["basis"] = "name_and_size_metadata_candidates"
        result["first_call_may_build_or_reuse_cache"] = True
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path, help="Atlas CLI executable")
    parser.add_argument("--db", required=True, type=Path, help="Existing Atlas SQLite database")
    parser.add_argument("--output", required=True, type=Path, help="New aggregate JSON report; existing files are not overwritten")
    parser.add_argument("--duplicates", action="store_true", help="Also build/reuse derived duplicate caches; this writes analysis cache tables")
    parser.add_argument("--exact-count", action="store_true", help="Also request an exact total for each search; may be expensive")
    parser.add_argument("--timeout", type=float, default=300, help="Seconds allowed per service request (default 300)")
    args = parser.parse_args()
    cli, db, output = args.cli.resolve(), args.db.resolve(), args.output.resolve()
    if not cli.is_file() or not db.is_file():
        parser.error("--cli and --db must name existing files")
    protected = {cli, db, Path(str(db) + "-wal"), Path(str(db) + "-shm"), Path(str(db) + "-journal")}
    if output in protected or output.exists() or not output.parent.is_dir():
        parser.error("--output must be a new file in an existing directory, separate from the database and its sidecars")
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    with cli.open("rb") as stream:
        digest = hashlib.sha256()
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    report = {
        "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "environment": {"platform": platform.platform(), "python": platform.python_version(), "cpu_count_visible": os.cpu_count(), "cli_sha256": digest.hexdigest()},
        "method": "One persistent CLI serve process; page size 250; first request followed by five repeated requests. No cache flush, cold-start guarantee or concurrent-load isolation.",
        "privacy": "Aggregate results only; no result names, paths, disk labels or entry IDs are written.",
        "exact_counts_requested": args.exact_count,
        "duplicate_cache_writes_enabled": args.duplicates,
        "search": [],
    }
    service = Service(cli, db, args.timeout)
    try:
        stats, duration = service.call("stats")
        report["catalog"] = {key: stats[key] for key in ("entries", "volumes", "total_bytes") if key in stats}
        report["stats_ms"] = duration
        for label, query in SEARCHES:
            row = measure(service, label, query)
            if args.exact_count:
                counted, duration = service.call("search", query=query, limit=250, offset=0)
                row["total_matches"] = counted["total"]
                row["exact_count_plus_page_ms"] = duration
            report["search"].append(row)
        if args.duplicates:
            report["duplicates"] = [measure(service, label, query, "duplicates") for label, query in DUPLICATES]
    finally:
        service.close()
    with output.open("x", encoding="utf-8") as stream:
        json.dump(report, stream, ensure_ascii=False, indent=2)
        stream.write("\n")
    print(json.dumps({"status": "passed", "search_cases": len(report["search"]), "duplicate_cases": len(report.get("duplicates", []))}))


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, KeyError) as error:
        print(json.dumps({"status": "failed", "error": type(error).__name__, "message": str(error)}), file=sys.stderr)
        sys.exit(1)
