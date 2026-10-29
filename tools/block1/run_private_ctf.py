#!/usr/bin/env python3
"""Read-only B1 checkpoint from two real CTFs.

Run:
 python tools/block1/run_private_ctf.py --probe /path/to/ctf_real_probe \
   --acceptance /path/to/ctf_real_acceptance --output /private/new/checkpoint \
   /path/to/e-books.CTF /path/to/Mp3-60Gb.CTF

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

def phase_status(r: dict[str, Any]) -> str:
    if r.get("exit_code") != 0:
        return "failed"
    data = r.get("report", {})
    if not isinstance(data, dict):
        return "unverified"
    phase = r.get("phase")
    if phase == "probe":
        datasets = data.get("datasets")
        if not isinstance(datasets, list) or len(datasets) != 2:
            return "unverified"
        return "passed" if all(
            d.get("report", {}).get("status") == "parsed"
            and d.get("report", {}).get("post_check", {}).get("source_unchanged") is True
            for d in datasets
        ) else "failed"
    if phase == "acceptance":
        datasets = data.get("reports")
        if not isinstance(datasets, list) or len(datasets) != 2:
            return "unverified"
        return "passed" if all(d.get("status") == "passed_available_checks" for d in datasets) else "failed"
    return "unverified"

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--probe", type=pathlib.Path, required=True)
    ap.add_argument("--acceptance", type=pathlib.Path, required=True)
    ap.add_argument("--output", type=pathlib.Path, required=True,
                    help="A new, nonexistent output directory, separate from CTFs")
    ap.add_argument("--timeout-seconds", type=int, default=1800)
    ap.add_argument("sources", type=pathlib.Path, nargs=2, metavar="CTF")
    args = ap.parse_args()
    if args.timeout_seconds < 10:
        ap.error("Timeout must be at least 10 seconds")
    sources = [p.resolve(strict=True) for p in args.sources]
    if sources[0] == sources[1]:
        ap.error("Provide two different source CTF paths")
    if not all(p.is_file() and p.suffix.lower() == ".ctf" for p in sources):
        ap.error("Both inputs must be existing regular .CTF files")
    for exe in [args.probe, args.acceptance]:
        if not exe.is_file():
            ap.error("Compile/download both native example executables before running")
    output = args.output.resolve()
    if output.exists() or output.is_symlink():
        ap.error("Output directory must not exist; never overwrite a previous checkpoint")
    if output in sources or output in [p.parent for p in sources]:
        ap.error("Output must not overwrite or coincide with a source")
    files = [
        {"ordinal": i+1, "file_label": p.name, "size_bytes": p.stat().st_size,
         "sha256_before": hash_file(p)} for i,p in enumerate(sources)
    ]
    output.mkdir(parents=True, exist_ok=False)
    start = time.perf_counter()
    phases: list[dict[str, Any]] = []
    try:
        for phase, exe in [("probe", args.probe), ("acceptance", args.acceptance)]:
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
    passed = all(x["unchanged"] for x in files) and len(phases) == 2 and all(
        phase_status(x) == "passed" for x in phases
    )
    summary = {
        "format": "whereisit-ai-b1-private-checkpoint-v1",
        "inputs_are_real_user_ctf": True,
        "private_bytes_exported": False,
        "source_files_written": False,
        "duration_seconds": round(time.perf_counter()-start,3),
        "files": files,
        "phases": [{"phase": x["phase"], "status": phase_status(x),
                    "exit_code": x.get("exit_code")} for x in phases],
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
        "", "Original source files were read-only; raw data stayed on this machine.",
        "", "| Input | Bytes | SHA-256 | Unchanged |",
        "|---|---:|---|---|"
    ]
    report.extend(
        "| {} | {} | {} | {} |".format(f["file_label"], f["size_bytes"], f["sha256_before"], f["unchanged"])
        for f in files
    )
    report += ["", "| Phase | Status |", "|---|---|"]
    report.extend("| {} | {} |".format(x["phase"],phase_status(x)) for x in phases)
    report += [
        "", "## Raw results", "",
        "The probe.json and acceptance.json contain aggregate measurements only.",
        "All catalog names, full paths and descriptions are intentionally absent.",
        "Original WhereIsIt acceptance and cross-file crash recovery remain unverified.",
        ""
    ]
    (output / "B1_REPORT.md").write_text("\n".join(report), encoding="utf-8")
    artifacts = ["probe.json","acceptance.json","B1_CHECKPOINT.json","B1_REPORT.md"]
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
