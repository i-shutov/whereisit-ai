"""Synthetic Windows-only stable publication gate contracts."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
PREPARER = ROOT / "tools" / "prepare_release_assets.py"
CHECKER = ROOT / "tools" / "qa" / "check_public_evidence.py"
VERSION = "1.0.0"
SHA = "0123456789abcdef" * 2 + "01234567"
RUN_ID = "123456789"
REPOSITORY = "example/whereisit"
SCOPE = {
    "ship_platforms": ["windows-x64"],
    "deferred_product_platforms": ["macos-arm64", "macos-x86_64", "linux"],
    "unsigned_windows_stable_approved": True,
    "usb_binding_enabled": False,
    "deferred_features": ["tabs_workspaces", "sunburst_archive_slices", "local_analyzer"],
}


class EguiStablePackagingContract(unittest.TestCase):
    def test_stable_selects_only_windows_setup_and_three_provenance_assets(self):
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)
            self._make_repo(repo)
            result = self._prepare(repo)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            assets = repo / "release-assets"
            self.assertEqual({p.name for p in assets.iterdir()}, {
                f"WhereIsItAI-egui-{VERSION}-windows-x64-setup.exe",
                "provenance.json", "ACCEPTANCE.json", "SHA256SUMS.txt",
            })
            provenance = json.loads((assets / "provenance.json").read_text(encoding="utf-8"))
            self.assertEqual(provenance["channel"], "stable")
            self.assertEqual(provenance["signing"], "unsigned")
            self.assertIs(provenance["unsigned_windows_stable_approved"], True)
            self.assertEqual(provenance["required_jobs"], {"test": "success", "windows": "success"})
            self.assertNotIn("macos", json.dumps(provenance["required_jobs"]))

    def test_windows_stable_does_not_require_macos_jobs_and_accepts_bom_inputs(self):
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)
            self._make_repo(repo, bom=True)
            result = self._prepare(repo)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_wrong_source_or_failed_required_job_blocks_stable(self):
        for case in ("source", "job"):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temp:
                repo = Path(temp)
                self._make_repo(repo)
                if case == "source":
                    path = repo / "verified-run.json"
                    run = json.loads(path.read_text(encoding="utf-8"))
                    run["head_sha"] = "f" * 40
                    path.write_text(json.dumps(run), encoding="utf-8")
                else:
                    path = repo / "verified-jobs.json"
                    jobs = json.loads(path.read_text(encoding="utf-8"))
                    jobs["jobs"][1]["conclusion"] = "failure"
                    path.write_text(json.dumps(jobs), encoding="utf-8")
                result = self._prepare(repo)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((repo / "release-assets").exists())

    def test_unsigned_policy_and_acceptance_gates_are_mandatory(self):
        for case in ("approval", "stable_ready", "catalogs", "open_gates", "jobs"):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temp:
                repo = Path(temp)
                self._make_repo(repo)
                path = repo / "release/ACCEPTANCE.json"
                acceptance = json.loads(path.read_text(encoding="utf-8"))
                if case == "approval":
                    acceptance["release_scope"]["unsigned_windows_stable_approved"] = False
                else:
                    if case == "stable_ready":
                        acceptance["stable_ready"] = False
                    elif case == "catalogs":
                        acceptance["personal_catalogs_in_release"] = True
                    elif case == "open_gates":
                        acceptance["known_open_gates"] = ["qa_pending"]
                    else:
                        acceptance["publication_requires_successful_jobs"].append("macos (macos-15)")
                path.write_text(json.dumps(acceptance), encoding="utf-8")
                result = self._prepare(repo)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((repo / "release-assets").exists())

    def test_public_evidence_privacy_gate_runs_in_stable_mode(self):
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)
            self._make_repo(repo)
            (repo / "docs/QA_EXPORT.md").write_text("synthetic fixture " + "a" * 64, encoding="utf-8")
            result = self._prepare(repo)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Private historical evidence must be redacted", result.stdout + result.stderr)
            self.assertFalse((repo / "release-assets").exists())

    def test_product_manifest_must_record_approved_unsigned_scope_and_checksum(self):
        for case in ("policy", "checksum"):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temp:
                repo = Path(temp)
                self._make_repo(repo)
                manifest = next((repo / "incoming").rglob("provenance.json"))
                if case == "policy":
                    product = json.loads(manifest.read_text(encoding="utf-8"))
                    product["signing"] = "signed"
                    manifest.write_text(json.dumps(product), encoding="utf-8")
                else:
                    installer = next((repo / "incoming").rglob("*.exe"))
                    installer.write_bytes(installer.read_bytes() + b"tamper")
                result = self._prepare(repo)
                self.assertNotEqual(result.returncode, 0)
                output = result.stdout + result.stderr
                self.assertIn("manifest identity, scope, or unsigned policy mismatch" if case == "policy"
                              else "Product checksum mismatch", output)
                self.assertFalse((repo / "release-assets").exists())

    def test_workflows_are_manual_stable_and_rc_publisher_skips_stable(self):
        stable = (ROOT / ".github/workflows/publish-windows-stable.yml").read_text(encoding="utf-8")
        rc = (ROOT / ".github/workflows/publish-validated-candidate.yml").read_text(encoding="utf-8")
        self.assertIn("workflow_dispatch:", stable)
        self.assertIn("branches: [release/windows-1.0-publish]", stable)
        self.assertIn("github.ref == 'refs/heads/main'", stable)
        self.assertIn("github.ref == 'refs/heads/release/windows-1.0-publish'", stable)
        self.assertIn("SOURCE_SHA: ${{ inputs.source_sha || github.sha }}", stable)
        self.assertIn("ref: ${{ inputs.source_sha || github.sha }}", stable)
        self.assertIn("source_sha:", stable)
        self.assertIn("native_run_id:", stable)
        self.assertIn("current main", stable)
        self.assertIn("actions/workflows/build.yml/runs?branch=main&head_sha=$SOURCE_SHA&event=push&status=completed", stable)
        self.assertIn("run.get('head_sha') == os.environ['SOURCE_SHA']", stable)
        self.assertIn("run.get('event') == 'push'", stable)
        self.assertIn("run.get('path') == '.github/workflows/build.yml'", stable)
        self.assertIn("run.get('conclusion') == 'success'", stable)
        self.assertIn("--stable", stable)
        self.assertIn("--draft --latest=false --notes-file docs/releases/v1.0.0.md", stable)
        self.assertIn("isPrerelease", stable)
        self.assertIn("--draft=false --latest", stable)
        self.assertIn("if [[ \"$version\" == '1.0.0' ]]", rc)
        self.assertIn("publish=false", rc)

    def _make_repo(self, repo: Path, *, bom: bool = False) -> None:
        (repo / "tools/qa").mkdir(parents=True)
        (repo / "release").mkdir()
        (repo / "docs/validation").mkdir(parents=True)
        (repo / "docs/releases").mkdir(parents=True)
        shutil.copyfile(PREPARER, repo / "tools/prepare_release_assets.py")
        shutil.copyfile(CHECKER, repo / "tools/qa/check_public_evidence.py")
        for report in ("QA_EXPORT.md", "STATUS_2026-10-28.md", "CTF_FORMAT.md", "VALIDATION.md"):
            (repo / "docs" / report).write_text("Synthetic release fixture.\n", encoding="utf-8")
        (repo / "docs/validation/database-package.json").write_text(
            json.dumps({"archive": "local-only-not-distributed", "entries": 1}) + "\n", encoding="utf-8")
        (repo / "docs/validation/performance.json").write_text(json.dumps({"samples": [1]}) + "\n", encoding="utf-8")
        (repo / "docs/releases/v1.0.0.md").write_text("Synthetic stable notes.\n", encoding="utf-8")
        (repo / "VERSION").write_text(VERSION + "\n", encoding="utf-8")
        acceptance = {"version": VERSION, "channel": "stable", "stable_ready": True,
                      "personal_catalogs_in_release": False, "known_open_gates": [],
                      "publication_requires_successful_jobs": ["test", "windows"],
                      "release_scope": SCOPE}
        (repo / "release/ACCEPTANCE.json").write_text(json.dumps(acceptance),
                                                       encoding="utf-8-sig" if bom else "utf-8")
        run = {"head_sha": SHA, "head_branch": "main", "conclusion": "success",
               "event": "push", "path": ".github/workflows/build.yml",
               "head_repository": {"full_name": REPOSITORY}}
        (repo / "verified-run.json").write_text(json.dumps(run), encoding="utf-8")
        (repo / "verified-jobs.json").write_text(json.dumps({"jobs": [
            {"name": "test", "conclusion": "success"},
            {"name": "windows", "conclusion": "success"},
            {"name": "macos (macos-15)", "conclusion": "cancelled"},
        ]}), encoding="utf-8")
        incoming = repo / "incoming/WhereIsItAI-egui-windows-x64-unsigned-candidate"
        incoming.mkdir(parents=True)
        installer_name = f"WhereIsItAI-egui-{VERSION}-windows-x64-setup.exe"
        installer = incoming / installer_name
        installer.write_bytes(b"synthetic setup bytes")
        checksum = f"{hashlib.sha256(installer.read_bytes()).hexdigest()}  {installer_name}\n"
        (incoming / "SHA256SUMS.txt").write_text(checksum, encoding="utf-8-sig" if bom else "ascii")
        product = {"schema": "whereisit-egui-product-candidate-v1", "channel": "stable",
                   "target_ui": "Rust/egui", "platform": "windows", "architecture": "x64",
                   "version": VERSION, "commit": SHA, "run_id": RUN_ID,
                   "installer_sha256": hashlib.sha256(installer.read_bytes()).hexdigest(),
                   "signing": "unsigned", "stable_ready": True,
                   "personal_catalogs_included": False, "release_scope": SCOPE}
        (incoming / "provenance.json").write_text(json.dumps(product), encoding="utf-8-sig" if bom else "utf-8")

    @staticmethod
    def _prepare(repo: Path) -> subprocess.CompletedProcess[str]:
        env = {**os.environ, "SOURCE_SHA": SHA, "CI_RUN_ID": RUN_ID,
               "GITHUB_REPOSITORY": REPOSITORY}
        return subprocess.run([sys.executable, str(repo / "tools/prepare_release_assets.py"), "--stable"],
                              cwd=repo, env=env, capture_output=True, text=True, check=False)


if __name__ == "__main__":
    unittest.main()
