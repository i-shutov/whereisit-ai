#!/usr/bin/env python3
"""Read-only B1 checkpoint from two real CTFs.

Run:
 python tools/block1/run_private_ctf.py --probe /path/to/ctf_real_probe \
   --acceptance /path/to/ctf_real_acceptance --ram-index /path/to/ctf_real_ram_index --output /private/new/checkpoint \
   /path/to/source-A.CTF /path/to/source-B.CTF

Produces aggregate-only JSON and Markdown. Never uploads any raw catalog bytes.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import pathlib
import subprocess
import sys
import time
from typing import Any

def hash_file(path: pathlib.Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(4 * 1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()

def execute(executable: pathlib.Path, sources: list[pathlib.Path], phase: str, timeout: int) -> dict[str, Any]:
    start = time.perf_counter()
    try:
        result = subprocess.run(
            [str(executable), *(str(p) for p in sources)],
            capture_output=True, text=True, check=False, timeout=timeout, errors="replace"
        )
    except (OSError, subprocess.TimeoutExpired) as e:
        return {"phase": phase, "status": "not_executed", "error_type": type(e).__name__,
                "elapsed_seconds": round(time.perf_counter() - start, 3)}
    # stderr may contain private paths: deliberately discard its contents.
    try:
        report = json.loads(result.stdout)
    except (ValueError, TypeError):
        report = {"status": "invalid_json_stdout"}
    return {
        "phase": phase,
        "exit_code": result.returncode,
        "elapsed_seconds": round(time.perf_counter() - start, 3),
        "report": report,
    }

def phase_status(r: dict[str, Any], expected: list[dict[str, Any]]) -> str:
    """Fail closed: success requires exact input identities and all fidelity gates.

    Native exit=0 and a 'parsed' status do NOT establish CTF/SQLite byte parity.
    This validator deliberately rejects partial/old schemas, dataset swaps and
    synthetic self-test markers rather than guessing about what was measured.
    """
    if r.get("status") == "not_executed":
        return "not_executed"
    if r.get("exit_code") != 0:
        return "failed"
    data = r.get("report")
    phase = r.get("phase")
    tool = {"probe": "ctf_real_probe_v1",
            "acceptance": "ctf_real_acceptance_v1",
            "ram_index": "b1_ram_index_v1"}.get(phase)
    key = "datasets" if phase == "probe" else "reports"
    if not tool or not isinstance(data, dict) or data.get("tool") != tool:
        return "unverified"
    if data.get("synthetic_self_test") is True:
        return "unverified"
    datasets = data.get(key)
    if not isinstance(datasets, list) or len(datasets) != len(expected):
        return "unverified"
    for i, item in enumerate(datasets):
        if not isinstance(item, dict):
            return "unverified"
        if phase in ("probe", "ram_index"):
            if item.get("dataset_number") != i + 1:
                return "unverified"
            entry = item.get("report" if phase == "probe" else "result")
        else:
            if item.get("dataset_number", i + 1) != i + 1:
                return "unverified"
            entry = item
        if not isinstance(entry, dict):
            return "unverified"
        identity = {"sha256": expected[i]["sha256_before"],
                    "size_bytes": expected[i]["size_bytes"]}
        if entry.get("source") != identity:
            return "failed"
        if phase == "probe":
            roundtrip = entry.get("ctf_roundtrip", {})
            diagnostic = entry.get("parser_diagnostics", {})
            post_check = entry.get("post_check")
            good = (entry.get("status") == "parsed"
                    and isinstance(roundtrip, dict)
                    and roundtrip.get("status") == "byte_identical"
                    and roundtrip.get("input_bytes") == identity["size_bytes"]
                    and roundtrip.get("output_bytes") == identity["size_bytes"]
                    and roundtrip.get("output_sha256") == identity["sha256"]
                    and isinstance(diagnostic, dict) and diagnostic.get("status") == "ok"
                    and isinstance(post_check, dict)
                    and post_check.get("source_unchanged") is True
                    and entry.get("original_ctf_read_only") is True)
        elif phase == "acceptance":
            queries = entry.get("queries")
            sidecars = entry.get("sidecars", {})
            good = (entry.get("status") == "passed_available_checks"
                    and entry.get("original_unchanged") is True
                    and entry.get("source_written") is False
                    and entry.get("raw_roundtrip_equal") is True
                    and entry.get("sqlite_roundtrip_equal") is True
                    and entry.get("export_rows_equal") is True
                    and isinstance(queries, list) and len(queries) >= 10
                    and all(isinstance(q, dict)
                            and q.get("first_page_and_count_equal") is True for q in queries)
                    and isinstance(sidecars, dict)
                    and all(sidecars.get(key) is True for key in
                            ("rename_reuses", "changed_ctf_invalidates",
                             "edit_base_conflict_detected")))
        else:
            cases = entry.get("cases")
            good = (entry.get("status") == "passed"
                    and entry.get("source_unchanged") is True
                    and isinstance(cases, list) and len(cases) >= 5
                    and all(isinstance(c, dict)
                            and c.get("first_page_equal") is True
                            and c.get("count_equal") is True for c in cases))
        if not good:
            return "failed"
    return "passed"

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--probe", type=pathlib.Path, required=True)
    ap.add_argument("--acceptance", type=pathlib.Path, required=True)
    ap.add_argument("--ram-index", type=pathlib.Path, required=True)
    ap.add_argument("--output", type=pathlib.Path, required=True,
                    help="A new, nonexistent output directory, separate from CTFs")
    ap.add_argument("--timeout-seconds", type=int, default=1800)
    ap.add_argument("--assert-real-inputs", action="store_true",
                    help="Operator-confirmed real CTF provenance (not inferred from filenames)")
    ap.add_argument("sources", type=pathlib.Path, nargs=2, metavar="CTF")
    args = ap.parse_args()
    if args.timeout_seconds < 10:
        ap.error("Timeout must be at least 10 seconds")
    sources = [p.resolve(strict=True) for p in args.sources]
    if sources[0] == sources[1]:
        ap.error("Provide two different source CTF paths")
    if not all(p.is_file() and p.suffix.lower() == ".ctf" for p in sources):
        ap.error("Both inputs must be existing regular .CTF files")
    for exe in [args.probe, args.acceptance, args.ram_index]:
        if not exe.is_file():
            ap.error("Compile/download both native example executables before running")
    output = args.output.resolve()
    if output.exists() or output.is_symlink():
        ap.error("Output directory must not exist; never overwrite a previous checkpoint")
    if output in sources or output in [p.parent for p in sources]:
        ap.error("Output must not overwrite or coincide with a source")
    files = [
        {"ordinal": i+1, "file_label": f"CTF-{i+1}", "size_bytes": p.stat().st_size,
         "sha256_before": hash_file(p)} for i,p in enumerate(sources)
    ]
    output.mkdir(parents=True, exist_ok=False)
    start = time.perf_counter()
    phases: list[dict[str, Any]] = []
    try:
        for phase, exe in [("probe", args.probe), ("acceptance", args.acceptance), ("ram_index", args.ram_index)]:
            result = execute(exe.resolve(), sources, phase, args.timeout_seconds)
            phases.append(result)
            (output / (phase + ".json")).write_text(
                json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2) + "\n", encoding="utf-8"
            )
    finally:
        for item, p in zip(files, sources):
            try:
                item["sha256_after"] = hash_file(p)
            except OSError:
                item["sha256_after"] = None
                item["recheck_status"] = "source_missing_or_unreadable"
            item["unchanged"] = item["sha256_before"] == item["sha256_after"]
    phase_results = [{"phase": p["phase"], "status": phase_status(p, files),
                      "exit_code": p.get("exit_code")} for p in phases]
    passed = all(x["unchanged"] for x in files) and len(phases) == 3 and all(
        p["status"] == "passed" for p in phase_results
    )
    summary = {
        "format": "whereisit-ai-b1-private-checkpoint-v1",
        "inputs_are_real_user_ctf": bool(args.assert_real_inputs),
        "input_origin_method": "explicit_operator_assertion" if args.assert_real_inputs else "unverified",
        "private_bytes_exported": False,
        "source_files_written": False,
        "duration_seconds": round(time.perf_counter()-start,3),
        "files": files,
        "phases": phase_results,
        "passed_available_checks": passed,
        "not_yet_tested": [
            "Original WhereIsIt opening of modified exports",
            "OS-cold/p95 GUI latencies and native index integration",
            "Full arbitrary-query all-row parity, including native-only categories"
        ],
    }
    (output / "B1_CHECKPOINT.json").write_text(
        json.dumps(summary, ensure_ascii=False, indent=2, sort_keys=True)+"\n", encoding="utf-8"
    )
    report = [
        "# Block 1: real CTF acceptance checkpoint", "",
        "**Status:** " + ("PASS for available checks" if passed else "INCOMPLETE / FAILED"),
        "", "The coordinator uses read-only source operations and verifies unchanged SHA-256 before/after.",
        "CTF input names are redacted. Real-data provenance requires an explicit operator assertion.",
        "", "| Input | Bytes | SHA-256 | Unchanged |",
        "|---|---:|---|---|"
    ]
    report.extend(
        "| {} | {} | {} | {} |".format(f["file_label"], f["size_bytes"], f["sha256_before"], f["unchanged"])
        for f in files
    )
    report += ["", "| Phase | Status |", "|---|---|"]
    report.extend("| {} | {} |".format(x["phase"],x["status"]) for x in phase_results)
    report += [
        "", "## Raw results", "",
        "The probe.json, acceptance.json and ram_index.json contain aggregate measurements only.",
        "All catalog names, full paths and descriptions are intentionally absent.",
        "Original WhereIsIt acceptance and cross-file crash recovery remain unverified.",
        ""
    ]
    (output / "B1_REPORT.md").write_text("\n".join(report), encoding="utf-8")
    artifacts = ["probe.json","acceptance.json","ram_index.json","B1_CHECKPOINT.json","B1_REPORT.md"]
    (output / "SHA256SUMS.txt").write_text(
        "\n".join("{}  {}".format(hash_file(output/name),name) for name in artifacts)+"\n",
        encoding="utf-8"
    )
    print(json.dumps({"status":"PASS" if passed else "INCOMPLETE",
       "checkpoint_dir":str(output),
       "all_source_hashes_unchanged":all(f["unchanged"] for f in files),
       "phase_statuses":summary["phases"]},ensure_ascii=False,indent=2))
    return 0 if passed else 1

if __name__ == "__main__":
    sys.exit(main())
