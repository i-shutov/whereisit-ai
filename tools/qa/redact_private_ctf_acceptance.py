#!/usr/bin/env python3
"""Create a strict, allowlisted public QA summary from TWO *local* private V3 checkpoints.

Never copies catalog names/paths, private file or CLI digests, row IDs, query text,
source bytes, unexpected error codes, untrusted exception text, or raw reports.
The original V3_ACCEPTANCE.json and private CTF files stay on the local machine.
"""
from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
import re
import sys

MAX_REPORT_BYTES = 4 * 1024 * 1024
HEX64 = re.compile(r"[0-9a-f]{64}\Z")


class Refused(Exception):
    """A fixed, non-sensitive error: never include private source material."""


def is_sha(value: object) -> bool:
    return isinstance(value, str) and HEX64.fullmatch(value) is not None


def positive_count(value: object) -> bool:
    return type(value) is int and 0 < value <= (2**63 - 1)


def check(value: object, present: bool = True) -> str:
    if not present:
        return "NOT_RUN"
    return "PASS" if value is True else "FAIL"


def phase_complete(data: dict, key: str) -> bool:
    phases = data.get("phases")
    if not isinstance(phases, dict) or not isinstance(phases.get(key), dict):
        return False
    metric = phases[key]
    return (type(metric.get("exit_code")) is int and metric["exit_code"] == 0
            and isinstance(metric.get("elapsed_ms"), (int, float))
            and type(metric["elapsed_ms"]) is not bool
            and math.isfinite(metric["elapsed_ms"]) and metric["elapsed_ms"] >= 0)


def checked_queries(report: dict) -> bool:
    queries = report.get("queries")
    return (isinstance(queries, list) and len(queries) == 4
            and all(isinstance(q, dict) and q.get("query_number") == i
                    and type(q.get("query_number")) is int
                    and q.get("query_text_omitted") is True
                    and q.get("page_and_count_agree") is True
                    and type(q.get("total_count")) is int
                    and q["total_count"] >= 0
                    and type(q.get("page_length")) is int
                    and 0 <= q["page_length"] <= q["total_count"]
                    for i, q in enumerate(queries, 1)))


def checked_exports(report: dict) -> str:
    exports = report.get("derived_export_artifacts")
    if exports is None:
        return "NOT_RUN"
    if not isinstance(exports, list) or len(exports) != 2:
        return "FAIL"
    for index, fmt in enumerate(("csv", "parquet")):
        item = exports[index]
        if (not isinstance(item, dict) or item.get("format") != fmt
                or not positive_count(item.get("bytes"))
                or not is_sha(item.get("sha256"))
                or item.get("exit_code") != 0):
            return "FAIL"
    # Files were created by the private tool; CSV-vs-Parquet equality was NOT checked.
    return "PASS"


def summarize(report: dict, label: str) -> dict:
    if not isinstance(report, dict) or report.get("format") != "whereisit-local-ctf-v3-acceptance-v1":
        raise Refused()
    if report.get("status") not in ("failed", "passed_available_checks"):
        raise Refused()
    source = report.get("source")
    if not isinstance(source, dict) or not is_sha(source.get("sha256")):
        raise Refused()
    original = source["sha256"]  # Used ONLY to verify identity; never emitted.
    after = report.get("source_sha256_after")
    source_evidenced = is_sha(after) and report.get("source_unchanged") is True
    source_same = source_evidenced and original == after
    original_bytes = source.get("size_bytes")
    counts_valid = positive_count(report.get("volumes")) and positive_count(report.get("entries"))
    ctf = report.get("ctf_roundtrip")
    roundtrip_present = isinstance(ctf, dict)
    roundtrip_valid = (roundtrip_present and ctf.get("byte_identical") is True
                       and ctf.get("output_sha256") == original
                       and ctf.get("output_bytes") == original_bytes
                       and positive_count(original_bytes))
    query_present = "queries" in report
    import_present = isinstance(report.get("phases"), dict) and "import_ctf" in report["phases"]
    validate_present = isinstance(report.get("phases"), dict) and "validate_catalog" in report["phases"]
    checks = {
        "source_stable": check(source_same, is_sha(after)),
        "sqlite_3_51_3": check(report.get("sqlite_runtime_version") == "3.51.3",
                               "sqlite_runtime_version" in report),
        "import_and_validate": check(counts_valid and phase_complete(report, "import_ctf")
                                     and phase_complete(report, "validate_catalog"),
                                     import_present or validate_present),
        "four_first_page_search_parities": check(checked_queries(report), query_present),
        "unchanged_ctf_roundtrip": check(roundtrip_valid, roundtrip_present),
        "csv_and_parquet_created_not_compared": checked_exports(report),
    }
    mandatory = ["source_stable", "sqlite_3_51_3", "import_and_validate",
                 "four_first_page_search_parities", "unchanged_ctf_roundtrip"]
    # A failed native checkpoint can never turn green through a subset of booleans.
    available_pass = (report["status"] == "passed_available_checks"
                      and all(checks[k] == "PASS" for k in mandatory)
                      and checks["csv_and_parquet_created_not_compared"] != "FAIL"
                      and report.get("source_written") is False
                      and report.get("test_original_whereisit_compatibility") is False)
    return {
        "source_label": label,
        "local_available_checks": "PASS" if available_pass else "FAIL",
        "counts": {"volumes": report["volumes"] if counts_valid else None,
                   "entries": report["entries"] if counts_valid else None},
        "checks": checks,
        "provenance_attested": False,
    }


def load_private_checkpoint(folder: Path) -> dict:
    # No source path ever appears in output or exceptions.
    report_file = folder / "V3_ACCEPTANCE.json"
    try:
        if not report_file.is_file() or report_file.stat().st_size > MAX_REPORT_BYTES:
            raise Refused()
        data = json.loads(report_file.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, ValueError):
        raise Refused() from None
    if not isinstance(data, dict):
        raise Refused()
    return data


def redact(first: dict, second: dict) -> dict:
    summaries = [summarize(first, "CTF-1"), summarize(second, "CTF-2")]
    # A repeated source is not a two-catalog acceptance test. Its SHA remains private.
    if first["source"]["sha256"] == second["source"]["sha256"]:
        raise Refused()
    same_cli = (is_sha(first.get("cli_sha256")) and is_sha(second.get("cli_sha256"))
                and first["cli_sha256"] == second["cli_sha256"])
    both_pass = same_cli and all(s["local_available_checks"] == "PASS" for s in summaries)
    return {
        "schema": "whereisit-public-ctf-qa-redacted-v1",
        "two_distinct_local_sources": True,
        "same_exact_cli_binary_verified_privately": bool(same_cli),
        "available_local_checks": "PASS" if both_pass else "FAIL",
        "inputs": summaries,
        "unverified_release_gates": {
            "modified_ctf_opened_in_original_whereisit": "NOT_RUN",
            "physical_gui_win10": "NOT_RUN",
            "physical_gui_win11": "NOT_RUN",
            "physical_gui_mac_arm64": "NOT_RUN",
            "physical_gui_mac_intel": "NOT_RUN",
            "real_upgrade_and_rollback": "NOT_RUN",
            "authenticode_and_apple_notarization": "NOT_RUN",
            "measured_target_performance_slo": "NOT_RUN",
            "opaque_0x10_meaning_confirmed": "NOT_RUN",
        },
        "stable_1_0_acceptance": "BLOCKED",
        "note": "Local inputs were not independently attested; only unchanged-byte roundtrip and listed checks may pass. This is not proof of modified-CTF compatibility, export equality, physical GUI, signing or SLO.",
        "redaction": "strict static allowlist; private hashes, paths, file names, entry IDs, query text, raw errors, logs and source contents excluded",
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Redact TWO private local CTF QA checkpoints")
    parser.add_argument("--first", required=True, type=Path, help="First private V3 checkpoint directory")
    parser.add_argument("--second", required=True, type=Path, help="Second private V3 checkpoint directory")
    parser.add_argument("--output", required=True, type=Path, help="NEW safe JSON output file")
    args = parser.parse_args(argv)
    try:
        destination = args.output.resolve()
        if any(destination.is_relative_to(folder.resolve()) for folder in (args.first, args.second)):
            raise Refused()
        first = load_private_checkpoint(args.first)
        second = load_private_checkpoint(args.second)
        result = redact(first, second)
        # 'x' is O_EXCL: refuse overwrite, even after a race with another writer.
        with args.output.open("x", encoding="utf-8") as stream:
            json.dump(result, stream, indent=2, sort_keys=True, ensure_ascii=True)
            stream.write("\n")
        print("Redacted summary written; available local checks: " + result["available_local_checks"])
        return 0 if result["available_local_checks"] == "PASS" else 1
    except (Refused, OSError, TypeError, ValueError):
        # NO exception details, even when source paths are embedded in OS errors.
        print("REFUSED: input invalid, destination unsafe, or existing output", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
