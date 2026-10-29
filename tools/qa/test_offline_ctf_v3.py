"""Synthetic-only contract tests for the private CTF runner: no private inputs."""
from __future__ import annotations
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
TOOL = ROOT / "tools" / "qa" / "offline_ctf_v3.py"

FAKE = """#!/usr/bin/env python3
import json, pathlib, sys
if sys.argv[1:] == ["--sqlite-version"]:
    print(__VERSION__)
    sys.exit(0)
request = json.loads(sys.argv[2])
name, db = request["op"], pathlib.Path(request["db"])
if name == "import_ctf":
    data = pathlib.Path(request["path"]).read_bytes()
    if __FAULT__ == "edit_source":
        pathlib.Path(request["path"]).write_bytes(b"changed outside CLI")
    db.write_bytes(data)
    result = {"ok": True, "data": {"volumes": 1}}
elif name == "stats":
    result = {"ok": True, "data": {"volumes": 1, "entries": 2}}
elif name == "validate_catalog":
    result = {"ok": True, "data": {"integrity": "ok"}}
elif name in ("search_fast", "search"):
    result = {"ok": True, "data": {"items": [{"id": 1}, {"id": 2}],
              "total": 2}}
elif name == "export_ctf":
    data = db.read_bytes()
    if __FAULT__ == "wrong_roundtrip":
        data = data + b"unexpected"
    pathlib.Path(request["path"]).write_bytes(data)
    result = {"ok": True, "data": {"exported": True}}
elif name in ("export_csv", "export_parquet"):
    pathlib.Path(request["path"]).write_bytes(b"synthetic only")
    result = {"ok": True, "data": {"exported": True}}
else:
    result = {"ok": False, "error": {"code": "invalid_operation"}}
print(json.dumps(result))
"""


def fixture(directory: pathlib.Path, fault: str = "", version: str = "3.51.3"):
    source = directory / "private-original.CTF"
    source.write_bytes(b"synthetic ctf fixture (not a real WhereIsIt catalog)")
    cli = directory / "fake_cli.py"
    cli.write_text(
        FAKE.replace("__VERSION__", repr(version)).replace("__FAULT__", repr(fault)),
        encoding="utf-8",
    )
    cli.chmod(0o700)
    return cli, source


def call(root: pathlib.Path, cli: pathlib.Path, source: pathlib.Path,
         output: pathlib.Path, extra=None):
    return subprocess.run(
        [sys.executable, str(TOOL), "--cli", str(cli), "--output",
         str(output), *(extra or []), str(source)],
        capture_output=True, text=True, cwd=root, check=False,
    )


class PrivateRunnerTest(unittest.TestCase):
    def test_success_immutable_redacted_checkpoint(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            cli, source = fixture(root)
            reportdir = root / "checkpoint"
            done = call(root, cli, source, reportdir)
            self.assertEqual(done.returncode, 0, done.stderr)
            report = json.loads((reportdir / "V3_ACCEPTANCE.json").read_text())
            self.assertEqual(report["status"], "passed_available_checks")
            self.assertTrue(report["source_unchanged"])
            self.assertTrue(report["ctf_roundtrip"]["byte_identical"])
            self.assertEqual(len(report["queries"]), 4)
            self.assertEqual(report["sqlite_runtime_version"], "3.51.3")
            self.assertTrue((reportdir / "SHA256SUMS.txt").is_file())
            encoded = (reportdir / "V3_ACCEPTANCE.json").read_text()
            self.assertNotIn("private-original.CTF", encoded)
            self.assertNotIn(str(root), encoded)
            failed = call(root, cli, source, reportdir)
            self.assertNotEqual(failed.returncode, 0, "Output overwrite allowed")

    def test_unpatched_sqlite_not_accepted_without_source_modification(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            cli, source = fixture(root, version="3.46.0")
            resultdir = root / "checkpoint"
            done = call(root, cli, source, resultdir)
            self.assertNotEqual(done.returncode, 0)
            report = json.loads((resultdir / "V3_ACCEPTANCE.json").read_text())
            self.assertEqual(report["status"], "failed")
            self.assertEqual(report["error_code"], "unpatched_sqlite")
            self.assertTrue(report["source_unchanged"])

    def test_export_mismatch_fails_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            cli, source = fixture(root, fault="wrong_roundtrip")
            dest = root / "checkpoint"
            done = call(root, cli, source, dest)
            self.assertEqual(done.returncode, 1)
            report = json.loads((dest / "V3_ACCEPTANCE.json").read_text())
            self.assertEqual(report["error_code"], "ctf_roundtrip_mismatch")
            self.assertFalse(report["ctf_roundtrip"]["byte_identical"])
            self.assertTrue(report["source_unchanged"])

    def test_external_source_edit_is_detected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            cli, source = fixture(root, fault="edit_source")
            dest = root / "checkpoint"
            done = call(root, cli, source, dest)
            self.assertEqual(done.returncode, 1)
            report = json.loads((dest / "V3_ACCEPTANCE.json").read_text())
            self.assertEqual(report["status"], "failed")
            self.assertEqual(report["error_code"], "source_changed")
            self.assertFalse(report["source_unchanged"])

    def test_full_exports_leave_only_aggregate_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            cli, source = fixture(root)
            dest = root / "checkpoint"
            done = call(root, cli, source, dest, ["--full-export"])
            self.assertEqual(done.returncode, 0, done.stderr)
            report = json.loads((dest / "V3_ACCEPTANCE.json").read_text())
            self.assertEqual([a["format"] for a in report["derived_export_artifacts"]],
                             ["csv", "parquet"])
            self.assertTrue(report["derived_temporary_files_removed_after_run"])


if __name__ == "__main__":
    unittest.main()
