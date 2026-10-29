"""Synthetic-only negative tests for the fail-closed B1 private CTF checkpoint."""
from __future__ import annotations

import hashlib
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools/block1/run_private_ctf.py"

# Each test stub receives the real paths, hashes the bytes it actually saw,
# and emits the same privacy-safe schema as the native B1 executable.
STUB = """#!/usr/bin/env python3
import hashlib
import json
import pathlib
import sys
mode = __MODE__
fault = __FAULT__
reports = []
for n, name in enumerate(sys.argv[1:], 1):
    path = pathlib.Path(name)
    raw = path.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    identity = {"sha256": digest, "size_bytes": len(raw)}
    if mode == "probe":
        report = {"source": identity, "status": "parsed",
                  "original_ctf_read_only": True,
                  "parser_diagnostics": {"status": "ok"},
                  "ctf_roundtrip": {"status": "byte_identical",
                                    "input_bytes": len(raw),
                                    "output_bytes": len(raw),
                                    "output_sha256": digest},
                  "post_check": {"source_unchanged": True}}
        if fault == "roundtrip_mismatch":
            report["ctf_roundtrip"]["status"] = "byte_difference"
        if fault == "diagnostics_missing":
            report["parser_diagnostics"] = {"status": "diagnostics_failed"}
        if fault == "bad_postcheck":
            report["post_check"] = None
        reports.append({"dataset_number": n, "report": report})
    elif mode == "acceptance":
        report = {"source": identity, "status": "passed_available_checks",
                  "original_unchanged": True, "source_written": False,
                  "raw_roundtrip_equal": True, "sqlite_roundtrip_equal": True,
                  "export_rows_equal": True,
                  "queries": [{"first_page_and_count_equal": True}] * 12,
                  "sidecars": {"rename_reuses": True,
                               "changed_ctf_invalidates": True,
                               "edit_base_conflict_detected": True}}
        if fault == "missing_queries":
            report["queries"] = []
        if fault == "sqlite_loss":
            report["sqlite_roundtrip_equal"] = False
        reports.append(report)
        if fault == "edit_source" and n == 1:
            path.write_bytes(b"external edit to synthetic test file")
    elif mode == "ram_index":
        result = {"source": identity, "status": "passed",
                  "source_unchanged": True,
                  "cases": [{"first_page_equal": True, "count_equal": True}] * 7}
        if fault == "wrong_page":
            result["cases"][0]["first_page_equal"] = False
        reports.append({"dataset_number": n, "result": result})
if fault == "wrong_source":
    reports[0]["report" if mode == "probe" else "result" if mode == "ram_index" else "source"] = (
        dict(reports[0].get("report", reports[0].get("result", {})), source={"sha256": "0" * 64,
        "size_bytes": 10}) if mode != "acceptance" else {"sha256": "0" * 64, "size_bytes": 10})
if fault == "swap_order":
    reports.reverse()
if fault == "exit9":
    sys.exit(9)
tool = {"probe": "ctf_real_probe_v1", "acceptance": "ctf_real_acceptance_v1",
        "ram_index": "b1_ram_index_v1"}[mode]
if fault == "wrong_tool":
    tool = "untrusted_old_schema"
output = {"tool": tool, "datasets" if mode == "probe" else "reports": reports}
if fault == "synthetic_marker":
    output["synthetic_self_test"] = True
print(json.dumps(output))
"""


def fake_tool(root: pathlib.Path, mode: str, fault: str = "") -> pathlib.Path:
    path = root / (mode + ".py")
    path.write_text(
        STUB.replace("__MODE__", repr(mode)).replace("__FAULT__", repr(fault)),
        encoding="utf-8",
    )
    path.chmod(0o700)
    return path


class CheckpointTest(unittest.TestCase):
    def fixture(self, root: pathlib.Path):
        a, b = root / "a.CTF", root / "b.CTF"
        a.write_bytes(b"synthetic source record A")
        b.write_bytes(b"synthetic source record B")
        return a, b

    def invoke(self, root, a, b, probe, acceptance, ram, output, *, real=False):
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--probe", str(probe),
             "--acceptance", str(acceptance), "--ram-index", str(ram),
             "--output", str(output),
             *(["--assert-real-inputs"] if real else []), str(a), str(b)],
            cwd=root, capture_output=True, text=True, check=False,
        )

    def run_fixture(self, faults=None, *, real=False):
        faults = faults or {}
        with tempfile.TemporaryDirectory(prefix="b1-tests-") as directory:
            root = pathlib.Path(directory)
            a, b = self.fixture(root)
            before = [a.read_bytes(), b.read_bytes()]
            tools = [fake_tool(root, mode, faults.get(mode, ""))
                     for mode in ("probe", "acceptance", "ram_index")]
            dest = root / "checkpoint"
            result = self.invoke(root, a, b, *tools, dest, real=real)
            self.assertTrue((dest / "B1_CHECKPOINT.json").exists(), result.stderr)
            report = json.loads((dest / "B1_CHECKPOINT.json").read_text())
            return result, report, dest.exists(), [a.read_bytes(), b.read_bytes()], before

    def test_success_source_fidelity_and_immutable_checkpoint(self):
        with tempfile.TemporaryDirectory(prefix="b1-tests-") as directory:
            root = pathlib.Path(directory)
            a, b = self.fixture(root)
            tools = [fake_tool(root, mode) for mode in ("probe", "acceptance", "ram_index")]
            dest = root / "checkpoint"
            result = self.invoke(root, a, b, *tools, dest)
            self.assertEqual(result.returncode, 0, result.stderr)
            report = json.loads((dest / "B1_CHECKPOINT.json").read_text())
            self.assertTrue(report["passed_available_checks"])
            self.assertFalse(report["inputs_are_real_user_ctf"])
            self.assertEqual(report["files"][0]["file_label"], "CTF-1")
            self.assertEqual(report["files"][1]["file_label"], "CTF-2")
            self.assertEqual(report["files"][0]["sha256_before"],
                             hashlib.sha256(a.read_bytes()).hexdigest())
            self.assertTrue(all(x["unchanged"] for x in report["files"]))
            self.assertTrue((dest / "SHA256SUMS.txt").exists())
            self.assertEqual(len(report["phases"]), 3)
            self.assertEqual(
                [p["status"] for p in report["phases"]], ["passed"] * 3)
            again = self.invoke(root, a, b, *tools, dest)
            self.assertNotEqual(again.returncode, 0, "Existing checkpoint was overwritten")

    def test_operator_real_input_flag_is_explicit_only(self):
        result, report, _, after, before = self.run_fixture(real=True)
        self.assertEqual(result.returncode, 0)
        self.assertTrue(report["inputs_are_real_user_ctf"])
        self.assertEqual(report["input_origin_method"], "explicit_operator_assertion")
        self.assertEqual(after, before)

    def test_external_edit_detected_even_if_native_program_claims_pass(self):
        result, report, _, after, before = self.run_fixture(
            {"acceptance": "edit_source"})
        self.assertEqual(result.returncode, 1)
        self.assertFalse(report["passed_available_checks"])
        self.assertFalse(report["files"][0]["unchanged"])
        self.assertTrue(report["files"][1]["unchanged"])
        self.assertNotEqual(after[0], before[0])

    def test_parsed_but_nonidentical_roundtrip_is_not_accepted(self):
        result, report, _, _, _ = self.run_fixture({"probe": "roundtrip_mismatch"})
        self.assertEqual(result.returncode, 1)
        self.assertEqual(report["phases"][0]["status"], "failed")

    def test_diagnostics_failure_is_not_accepted_as_complete(self):
        result, report, _, _, _ = self.run_fixture({"probe": "diagnostics_missing"})
        self.assertEqual(result.returncode, 1)
        self.assertEqual(report["phases"][0]["status"], "failed")

    def test_malformed_native_postcheck_never_drops_failure_checkpoint(self):
        result, report, _, _, _ = self.run_fixture({"probe": "bad_postcheck"})
        self.assertEqual(result.returncode, 1)
        self.assertEqual(report["phases"][0]["status"], "failed")

    def test_wrong_source_identity_fails_even_with_green_tool_status(self):
        result, report, _, _, _ = self.run_fixture({"probe": "wrong_source"})
        self.assertEqual(result.returncode, 1)
        self.assertEqual(report["phases"][0]["status"], "failed")

    def test_reversed_dataset_order_cannot_claim_pass(self):
        result, report, _, _, _ = self.run_fixture({"ram_index": "swap_order"})
        self.assertEqual(result.returncode, 1)
        self.assertEqual(report["phases"][2]["status"], "unverified")

    def test_acceptance_without_query_or_sqlite_parity_fails(self):
        for fault in ("missing_queries", "sqlite_loss"):
            with self.subTest(fault=fault):
                result, report, _, _, _ = self.run_fixture({"acceptance": fault})
                self.assertEqual(result.returncode, 1)
                self.assertEqual(report["phases"][1]["status"], "failed")

    def test_ram_index_page_mismatch_fails(self):
        result, report, _, _, _ = self.run_fixture({"ram_index": "wrong_page"})
        self.assertEqual(result.returncode, 1)
        self.assertEqual(report["phases"][2]["status"], "failed")

    def test_unknown_tool_schema_cannot_claim_pass(self):
        result, report, _, _, _ = self.run_fixture({"ram_index": "wrong_tool"})
        self.assertEqual(result.returncode, 1)
        self.assertEqual(report["phases"][2]["status"], "unverified")

    def test_synthetic_selftest_output_cannot_claim_real_acceptance(self):
        result, report, _, _, _ = self.run_fixture({"acceptance": "synthetic_marker"})
        self.assertEqual(result.returncode, 1)
        self.assertEqual(report["phases"][1]["status"], "unverified")

    def test_partial_failures_still_write_checkpoint(self):
        result, report, _, _, _ = self.run_fixture(
            {"probe": "roundtrip_mismatch", "acceptance": "exit9"})
        self.assertEqual(result.returncode, 1)
        self.assertFalse(report["passed_available_checks"])
        self.assertEqual(len(report["phases"]), 3)
        self.assertEqual(report["phases"][0]["status"], "failed")
        self.assertEqual(report["phases"][1]["status"], "failed")


if __name__ == "__main__":
    unittest.main()
