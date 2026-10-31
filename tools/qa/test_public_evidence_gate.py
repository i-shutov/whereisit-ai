"""Generated-only negative regressions for the mandatory RC3 publication privacy gate.

Never copy a real owner filename, SHA, CTF, SQLite, or path into a fixture.
"""
from __future__ import annotations

import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

CHECKER = Path(__file__).with_name("check_public_evidence.py")
REPORTS = (
    "QA_EXPORT.md", "STATUS_2026-10-28.md", "CTF_FORMAT.md", "VALIDATION.md"
)


class PublicEvidenceReleaseGate(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        qa_dir = self.root / "tools" / "qa"
        qa_dir.mkdir(parents=True)
        shutil.copyfile(CHECKER, qa_dir / "check_public_evidence.py")
        docs = self.root / "docs"
        (docs / "validation").mkdir(parents=True)
        for name in REPORTS:
            (docs / name).write_text("Synthetic report, no catalog metadata.\n", encoding="utf-8")
        (docs / "validation" / "database-package.json").write_text(
            json.dumps({"archive": "local-only-not-distributed", "entries": 5}) + "\n",
            encoding="utf-8",
        )
        (docs / "validation" / "performance.json").write_text(
            json.dumps({"samples": [1, 2, 3]}) + "\n", encoding="utf-8"
        )

    def run_checker(self):
        return subprocess.run(
            [sys.executable, str(self.root / "tools" / "qa" / "check_public_evidence.py")],
            cwd=self.root, capture_output=True, text=True, check=False,
        )

    def test_safe_synthetic_reports_pass(self):
        p = self.run_checker()
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertIn("PASS:", p.stdout)

    def test_uppercase_and_lowercase_private_like_digests_fail_without_leaking(self):
        for digest in ("ab" * 32, "CD" * 32):
            with self.subTest(case=digest[:2]):
                target = self.root / "docs" / "validation" / "performance.json"
                target.write_text(json.dumps({"private_digest": digest}), encoding="utf-8")
                p = self.run_checker()
                self.assertNotEqual(p.returncode, 0)
                self.assertNotIn(digest, p.stdout + p.stderr)

    def test_private_archive_reference_fails_without_leaking(self):
        path = self.root / "docs" / "validation" / "database-package.json"
        candidate = "/synthetic/private/backup-volume.CTF"
        path.write_text(json.dumps({"archive": candidate}), encoding="utf-8")
        p = self.run_checker()
        self.assertNotEqual(p.returncode, 0)
        self.assertNotIn(candidate, p.stdout + p.stderr)

    def test_historical_markdown_report_is_checked(self):
        target = self.root / "docs" / "VALIDATION.md"
        target.write_text("A private-like identifier: " + "12" * 32, encoding="utf-8")
        p = self.run_checker()
        self.assertNotEqual(p.returncode, 0)
        self.assertNotIn("12" * 32, p.stdout + p.stderr)

    def test_invalid_report_json_is_rejected(self):
        (self.root / "docs" / "validation" / "performance.json").write_text(
            "{ invalid synthetic JSON", encoding="utf-8"
        )
        self.assertNotEqual(self.run_checker().returncode, 0)


if __name__ == "__main__":
    unittest.main()
