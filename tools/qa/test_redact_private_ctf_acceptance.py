"""Synthetic-only tests of the strict public summary allowlist."""
from __future__ import annotations

import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

from redact_private_ctf_acceptance import redact, Refused

TOOL = pathlib.Path(__file__).with_name("redact_private_ctf_acceptance.py")


def fixture(digest: str) -> dict:
    return {
        "format": "whereisit-local-ctf-v3-acceptance-v1",
        "status": "passed_available_checks",
        "source": {"sha256": digest, "size_bytes": 1024, "private_path": "SECRET_FILE_NAME"},
        "source_sha256_after": digest,
        "source_unchanged": True,
        "cli_sha256": "c" * 64,
        "source_written": False,
        "test_original_whereisit_compatibility": False,
        "sqlite_runtime_version": "3.51.3",
        "volumes": 2,
        "entries": 16,
        "phases": {
            "import_ctf": {"elapsed_ms": 12.3, "exit_code": 0},
            "validate_catalog": {"elapsed_ms": 9.0, "exit_code": 0},
        },
        "queries": [{"query_number": i, "query_text_omitted": True,
                     "page_and_count_agree": True, "total_count": 9,
                     "page_length": 5, "private_names": "SECRET_FILE_NAME"}
                    for i in range(1, 5)],
        "ctf_roundtrip": {"byte_identical": True,
                          "output_sha256": digest, "output_bytes": 1024},
        "private_error": "SECRET_PATH_CTF_AND_SQLITE",
        "derived_export_artifacts": [
            {"format": "csv", "bytes": 60, "sha256": "1" * 64, "exit_code": 0},
            {"format": "parquet", "bytes": 120, "sha256": "2" * 64, "exit_code": 0},
        ],
    }


class RedactedAcceptanceTests(unittest.TestCase):
    def test_two_distinct_sources_strict_allowlist(self):
        a, b = fixture("a" * 64), fixture("b" * 64)
        a["secret_random_sha"] = "PRIVATE_SHA_SECRET_TOKEN"
        summary = redact(a, b)
        raw = json.dumps(summary)
        for forbidden in ("SECRET", "private", "source_sha256_after", "query_number",
                          "a" * 64, "b" * 64, "c" * 64, "1" * 64, "2" * 64):
            # The static explanatory note uses 'private' intentionally.
            if forbidden == "private":
                continue
            self.assertNotIn(forbidden, raw)
        self.assertEqual(summary["available_local_checks"], "PASS")
        self.assertEqual(summary["stable_1_0_acceptance"], "BLOCKED")
        self.assertTrue(all(g == "NOT_RUN" for g in summary["unverified_release_gates"].values()))
        self.assertEqual(summary["inputs"][0]["counts"], {"volumes": 2, "entries": 16})

    def test_fail_and_missing_checks_cannot_claim_pass(self):
        a, b = fixture("a" * 64), fixture("b" * 64)
        b["status"] = "failed"
        b["error_code"] = "SECRET_LOCAL_FILE_NAME_AND_PATH"
        self.assertEqual(redact(a, b)["available_local_checks"], "FAIL")
        b["status"] = "passed_available_checks"
        del b["queries"]
        summary = redact(a, b)
        self.assertEqual(summary["available_local_checks"], "FAIL")
        self.assertEqual(summary["inputs"][1]["checks"]["four_first_page_search_parities"], "NOT_RUN")
        b["queries"] = a["queries"]
        b["ctf_roundtrip"]["output_sha256"] = "f" * 64
        self.assertEqual(redact(a, b)["available_local_checks"], "FAIL")
        b["ctf_roundtrip"]["output_sha256"] = "b" * 64
        b["source_sha256_after"] = "e" * 64
        self.assertEqual(redact(a, b)["available_local_checks"], "FAIL")

    def test_independent_field_parity_is_optional_but_never_implicitly_passes(self):
        a, b = fixture("a" * 64), fixture("b" * 64)
        initial = redact(a, b)
        self.assertEqual(initial["inputs"][0]["checks"]["csv_parquet_all_15_fields_equal"],
                         "NOT_RUN")
        self.assertEqual(initial["available_local_checks"], "PASS")
        a["derived_export_parity"] = {
            "status": "passed", "all_values_equal": True, "rows": 16,
            "columns": 15,
            "checker": "independent_duckdb_row_and_field_comparison",
            "untrusted_path": "PRIVATE_PATH_SHOULD_NOT_BE_COPIED",
        }
        actual = redact(a, b)
        self.assertEqual(actual["inputs"][0]["checks"]["csv_parquet_all_15_fields_equal"],
                         "PASS")
        self.assertNotIn("PRIVATE_PATH_SHOULD_NOT_BE_COPIED", json.dumps(actual))
        a["derived_export_parity"]["rows"] = 15
        failed = redact(a, b)
        self.assertEqual(failed["available_local_checks"], "FAIL")
        self.assertEqual(failed["inputs"][0]["checks"]["csv_parquet_all_15_fields_equal"],
                         "FAIL")
        a["derived_export_parity"] = {"status": "failed"}
        self.assertEqual(redact(a, b)["available_local_checks"], "FAIL")

    def test_different_source_and_same_cli_required(self):
        a, b = fixture("a" * 64), fixture("a" * 64)
        with self.assertRaises(Refused):
            redact(a, b)
        b["source"]["sha256"] = "b" * 64
        b["source_sha256_after"] = "b" * 64
        b["ctf_roundtrip"]["output_sha256"] = "b" * 64
        b["cli_sha256"] = "d" * 64
        self.assertEqual(redact(a, b)["available_local_checks"], "FAIL")

    def test_cli_redaction_and_no_clobber(self):
        with tempfile.TemporaryDirectory() as work:
            root = pathlib.Path(work)
            a, b = root / "private-A", root / "private-B"
            a.mkdir()
            b.mkdir()
            (a / "V3_ACCEPTANCE.json").write_text(json.dumps(fixture("a" * 64)))
            (b / "V3_ACCEPTANCE.json").write_text(json.dumps(fixture("b" * 64)))
            dest = root / "public.json"
            args = [sys.executable, str(TOOL), "--first", str(a), "--second", str(b), "--output", str(dest)]
            result = subprocess.run(args, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(dest.read_text())["available_local_checks"], "PASS")
            self.assertNotIn(str(root), dest.read_text())
            self.assertNotIn("SECRET", result.stdout + result.stderr + dest.read_text())
            once = dest.read_bytes()
            again = subprocess.run(args, capture_output=True, text=True, check=False)
            self.assertEqual(again.returncode, 2)
            self.assertEqual(dest.read_bytes(), once)
            # Even a private input directory cannot be used as a summary destination.
            nested = subprocess.run(args[:-1] + [str(a / "unsafe.json")], capture_output=True, text=True, check=False)
            self.assertEqual(nested.returncode, 2)
            self.assertFalse((a / "unsafe.json").exists())
            # Missing input checkpoint / invalid JSON must not echo private paths.
            (b / "V3_ACCEPTANCE.json").write_text("invalid SECRET_PATH_CTF")
            refused = subprocess.run(args[:-1] + [str(root / "other.json")], capture_output=True, text=True, check=False)
            self.assertEqual(refused.returncode, 2)
            self.assertNotIn(str(root), refused.stderr)
            self.assertNotIn("SECRET", refused.stderr)
            self.assertFalse((root / "other.json").exists())


if __name__ == "__main__":
    unittest.main()
