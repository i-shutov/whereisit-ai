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
import textwrap
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
    def test_stable_publication_branch_guards_execute_workflow_bash(self):
        workflow = (ROOT / ".github/workflows/publish-windows-stable.yml").read_text(encoding="utf-8")
        bash = shutil.which("bash")
        if not bash and os.name == "nt":
            git_bash = Path("C:/Program Files/Git/bin/bash.exe")
            if git_bash.is_file():
                bash = str(git_bash)
        if not bash:
            self.skipTest("bash is required to execute the stable workflow guards")

        regex_guard = next(line.strip() for line in workflow.splitlines()
                           if '[[ "$PUBLICATION_EVENT_REF" =~' in line)
        version_step = workflow.split("      - name: Resolve stable version and refuse existing release or tag", 1)[1]
        version_step = version_step.split("      - name: Download the validated Windows product artifact", 1)[0]
        version_lines = version_step.splitlines()
        start = next(i for i, line in enumerate(version_lines)
                     if 'version="$(tr -d' in line)
        end = next(i for i in range(start, len(version_lines))
                   if '[[ "$GITHUB_REF" == "refs/heads/$PUBLICATION_EVENT_REF" ]]' in version_lines[i])
        self.assertEqual(version_lines[end + 1].strip(), "fi")
        version_guard = textwrap.dedent("\n".join(version_lines[start:end + 2]))
        script = "\n".join(("set -euo pipefail", regex_guard,
                             '[[ "$GITHUB_REF" == "refs/heads/$PUBLICATION_EVENT_REF" ]]',
                             '[[ "$GITHUB_SHA" == "$SOURCE_SHA" ]]', version_guard, ""))

        def run(ref: str, github_ref: str | None = None) -> subprocess.CompletedProcess[str]:
            env = {**os.environ, "GITHUB_EVENT_NAME": "create", "GITHUB_REF": github_ref or f"refs/heads/{ref}",
                   "PUBLICATION_EVENT_REF": ref, "GITHUB_SHA": SHA, "SOURCE_SHA": SHA}
            with tempfile.TemporaryDirectory() as temp:
                (Path(temp) / "VERSION").write_text("1.1.1\n", encoding="utf-8")
                return subprocess.run([bash, "-c", script], cwd=temp, env=env,
                                      capture_output=True, text=True)

        for ref in ("release/windows-1.1-publish", "release/windows-1.1.1-publish"):
            with self.subTest(ref=ref):
                result = run(ref)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

        for ref in ("release/windows-1.1.2-publish", "release/windows-1.2-publish",
                    "release/windows-1.1.1.0-publish", "release/windows-1.1.1-publish-extra"):
            with self.subTest(ref=ref):
                self.assertNotEqual(run(ref).returncode, 0, ref)
        with self.subTest(ref="corrupted GitHub ref"):
            self.assertNotEqual(run("release/windows-1.1.1-publish", "refs/heads/release/windows-1.1-publish").returncode, 0)

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

    def test_unshipped_macos_run_states_do_not_block_successful_windows_gates(self):
        cases = (
            ("queued", None, "queued", None),
            ("in_progress", None, "queued", None),
            ("completed", "failure", "completed", "failure"),
            ("completed", "cancelled", "completed", "cancelled"),
        )
        for run_status, conclusion, mac_status, mac_conclusion in cases:
            with self.subTest(run_status=run_status, mac_status=mac_status), tempfile.TemporaryDirectory() as temp:
                repo = Path(temp)
                self._make_repo(repo)
                run_path = repo / "verified-run.json"
                run = json.loads(run_path.read_text(encoding="utf-8"))
                run["status"] = run_status
                run["conclusion"] = conclusion
                run_path.write_text(json.dumps(run), encoding="utf-8")
                jobs_path = repo / "verified-jobs.json"
                jobs = json.loads(jobs_path.read_text(encoding="utf-8"))
                jobs["jobs"][2]["status"] = mac_status
                jobs["jobs"][2]["conclusion"] = mac_conclusion
                jobs_path.write_text(json.dumps(jobs), encoding="utf-8")
                result = self._prepare(repo)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                provenance = json.loads((repo / "release-assets/provenance.json").read_text(encoding="utf-8"))
                self.assertEqual(provenance["required_jobs"], {"test": "success", "windows": "success"})

    def test_windows_11_stable_accepts_empty_deferred_features(self):
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)
            scope = {**SCOPE, "target_version": "1.1.0", "deferred_features": [],
                     "deferred_platforms": ["macos", "linux"]}
            self._make_repo(repo, version="1.1.0", scope=scope)
            result = self._prepare(repo)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            provenance = json.loads((repo / "release-assets/provenance.json").read_text())
            self.assertEqual(provenance["version"], "1.1.0")
            self.assertEqual(provenance["release_scope"]["deferred_features"], [])

    def test_unready_stable_and_scope_version_mismatch_are_rejected(self):
        for case in ("not_ready", "acceptance_version", "scope_version"):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temp:
                repo = Path(temp)
                scope = {**SCOPE, "target_version": "1.1.0", "deferred_features": [],
                         "deferred_platforms": ["macos", "linux"]}
                self._make_repo(repo, version="1.1.0", scope=scope)
                path = repo / "release/ACCEPTANCE.json"
                acceptance = json.loads(path.read_text(encoding="utf-8"))
                if case == "not_ready":
                    acceptance["stable_ready"] = False
                elif case == "acceptance_version":
                    acceptance["version"] = "1.0.0"
                else:
                    acceptance["release_scope"]["target_version"] = "1.0.0"
                path.write_text(json.dumps(acceptance), encoding="utf-8")
                result = self._prepare(repo)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((repo / "release-assets").exists())

    def test_prerelease_and_malformed_versions_are_rejected_for_stable(self):
        for version in ("1.1.0-rc.1", "one.point.one"):
            with self.subTest(version=version), tempfile.TemporaryDirectory() as temp:
                repo = Path(temp)
                self._make_repo(repo, version=version)
                result = self._prepare(repo)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("numeric stable VERSION", result.stdout + result.stderr)
                self.assertFalse((repo / "release-assets").exists())

    def test_spoofed_or_malformed_run_identity_blocks_stable(self):
        for case in ("source", "repository", "branch", "event", "workflow_path", "run_id", "run_state", "malformed_status"):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temp:
                repo = Path(temp)
                self._make_repo(repo)
                path = repo / "verified-run.json"
                run = json.loads(path.read_text(encoding="utf-8"))
                if case == "source":
                    run["head_sha"] = "f" * 40
                elif case == "repository":
                    run["head_repository"]["full_name"] = "attacker/fork"
                elif case == "branch":
                    run["head_branch"] = "attacker"
                elif case == "event":
                    run["event"] = "pull_request"
                elif case == "workflow_path":
                    run["path"] = ".github/workflows/untrusted.yml"
                elif case == "run_id":
                    run["id"] = "987654321"
                elif case == "run_state":
                    run["conclusion"] = "timed_out"
                elif case == "malformed_status":
                    run["status"] = "queued"
                path.write_text(json.dumps(run), encoding="utf-8")
                result = self._prepare(repo)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((repo / "release-assets").exists())

    def test_required_test_and_windows_jobs_must_be_unique_completed_successes(self):
        cases = ("failure", "cancelled", "queued", "in_progress_success", "missing", "duplicate", "status_missing")
        for required in ("test", "windows"):
            for case in cases:
                with self.subTest(required=required, case=case), tempfile.TemporaryDirectory() as temp:
                    repo = Path(temp)
                    self._make_repo(repo)
                    jobs_path = repo / "verified-jobs.json"
                    jobs = json.loads(jobs_path.read_text(encoding="utf-8"))
                    matches = [job for job in jobs["jobs"] if job["name"] == required]
                    if case == "missing":
                        jobs["jobs"].remove(matches[0])
                    elif case == "duplicate":
                        jobs["jobs"].append(dict(matches[0]))
                    elif case == "failure":
                        matches[0]["conclusion"] = "failure"
                    elif case == "cancelled":
                        matches[0]["conclusion"] = "cancelled"
                    elif case == "in_progress_success":
                        matches[0]["status"] = "in_progress"
                    elif case == "status_missing":
                        del matches[0]["status"]
                    else:
                        matches[0]["status"] = "queued"
                        matches[0]["conclusion"] = None
                    jobs_path.write_text(json.dumps(jobs), encoding="utf-8")
                    result = self._prepare(repo)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertFalse((repo / "release-assets").exists())

    def test_actual_publisher_python_gate_allows_optional_macos_failure_only(self):
        workflow = (ROOT / ".github/workflows/publish-windows-stable.yml").read_text(encoding="utf-8")
        anchor = workflow.index("- name: Verify current main and successful source-bound Windows CI")
        marker = "python3 - <<'PY'\n"
        block_start = workflow.index(marker, anchor) + len(marker)
        block_end = workflow.index("\n          PY", block_start)
        script = textwrap.dedent(workflow[block_start:block_end])
        cases = (
            ("optional_mac_failed", "completed", "failure", None),
            ("optional_mac_queued_run", "queued", None, "mac_queued"),
            ("optional_mac_queued", "in_progress", None, "mac_queued"),
            ("required_missing", "completed", "failure", "missing"),
            ("required_failed", "completed", "failure", "failure"),
            ("required_cancelled", "completed", "cancelled", "cancelled"),
            ("required_duplicate", "completed", "failure", "duplicate"),
            ("required_status_missing", "completed", "failure", "status_missing"),
            ("required_in_progress_success", "completed", "failure", "in_progress_success"),
            ("spoofed_source", "completed", "failure", "source"),
            ("spoofed_path", "completed", "failure", "path"),
            ("spoofed_repository", "completed", "failure", "repository"),
            ("spoofed_event", "completed", "failure", "event"),
            ("spoofed_branch", "completed", "failure", "branch"),
            ("spoofed_run_id", "completed", "failure", "run_id"),
            ("queued_with_conclusion", "queued", "success", None),
            ("in_progress_with_conclusion", "in_progress", "success", None),
            ("unknown_run_state", "unknown", None, None),
            ("malformed_completed_conclusion", "completed", "timed_out", None),
        )
        for case, run_status, conclusion, mutation in cases:
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temp:
                repo = Path(temp)
                run = {"id": RUN_ID, "head_sha": SHA, "head_branch": "main", "status": run_status,
                       "conclusion": conclusion, "event": "push", "path": ".github/workflows/build.yml",
                       "head_repository": {"full_name": REPOSITORY}}
                if mutation == "source":
                    run["head_sha"] = "f" * 40
                elif mutation == "path":
                    run["path"] = ".github/workflows/untrusted.yml"
                elif mutation == "repository":
                    run["head_repository"]["full_name"] = "attacker/fork"
                elif mutation == "event":
                    run["event"] = "pull_request"
                elif mutation == "branch":
                    run["head_branch"] = "attacker"
                elif mutation == "run_id":
                    run["id"] = "987654321"
                jobs = [
                    {"name": "test", "status": "completed", "conclusion": "success"},
                    {"name": "windows", "status": "completed", "conclusion": "success"},
                    {"name": "macos (macos-15)", "status": "completed", "conclusion": "failure"},
                ]
                if mutation == "mac_queued":
                    jobs[2]["status"] = "queued"
                    jobs[2]["conclusion"] = None
                elif mutation == "missing":
                    jobs = [job for job in jobs if job["name"] != "windows"]
                elif mutation == "failure":
                    jobs[1]["conclusion"] = "failure"
                elif mutation == "cancelled":
                    jobs[1]["conclusion"] = "cancelled"
                elif mutation == "duplicate":
                    jobs.append(dict(jobs[1]))
                elif mutation == "status_missing":
                    del jobs[1]["status"]
                elif mutation == "in_progress_success":
                    jobs[1]["status"] = "in_progress"
                (repo / "verified-run.json").write_text(json.dumps(run), encoding="utf-8")
                (repo / "verified-jobs.json").write_text(json.dumps({"jobs": jobs}), encoding="utf-8")
                env = {**os.environ, "SOURCE_SHA": SHA, "CI_RUN_ID": RUN_ID,
                       "GITHUB_REPOSITORY": REPOSITORY}
                result = subprocess.run([sys.executable, "-c", script], cwd=repo, env=env,
                                        capture_output=True, text=True, check=False)
                if case.startswith("optional_mac_"):
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                else:
                    self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)

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
        for case in ("policy", "scope", "checksum"):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temp:
                repo = Path(temp)
                self._make_repo(repo)
                manifest = next((repo / "incoming").rglob("provenance.json"))
                if case == "policy":
                    product = json.loads(manifest.read_text(encoding="utf-8"))
                    product["signing"] = "signed"
                    manifest.write_text(json.dumps(product), encoding="utf-8")
                elif case == "scope":
                    product = json.loads(manifest.read_text(encoding="utf-8"))
                    product["release_scope"]["target_version"] = "9.9.9"
                    manifest.write_text(json.dumps(product), encoding="utf-8")
                else:
                    installer = next((repo / "incoming").rglob("*.exe"))
                    installer.write_bytes(installer.read_bytes() + b"tamper")
                result = self._prepare(repo)
                self.assertNotEqual(result.returncode, 0)
                output = result.stdout + result.stderr
                self.assertIn("manifest identity, scope, or unsigned policy mismatch" if case in ("policy", "scope")
                              else "Product checksum mismatch", output)
                self.assertFalse((repo / "release-assets").exists())

    def test_workflows_are_manual_stable_and_rc_publisher_skips_stable(self):
        stable = (ROOT / ".github/workflows/publish-windows-stable.yml").read_text(encoding="utf-8")
        rc = (ROOT / ".github/workflows/publish-validated-candidate.yml").read_text(encoding="utf-8")
        self.assertIn("workflow_dispatch:", stable)
        self.assertIn("  create:", stable)
        self.assertIn("github.event_name == 'create'", stable)
        self.assertIn("github.event.ref_type == 'branch'", stable)
        self.assertIn(r"release/windows-([0-9]+)\.([0-9]+)(\.([0-9]+))?-publish", stable)
        self.assertIn("github.ref == 'refs/heads/main'", stable)
        self.assertIn('expected_major_minor_branch="release/windows-${version%.*}-publish"', stable)
        self.assertIn('expected_full_version_branch="release/windows-$version-publish"', stable)
        self.assertIn('"$PUBLICATION_EVENT_REF" == "$expected_major_minor_branch" ||', stable)
        self.assertIn('"$PUBLICATION_EVENT_REF" == "$expected_full_version_branch"', stable)
        self.assertIn("SOURCE_SHA: ${{ inputs.source_sha || github.sha }}", stable)
        self.assertIn("PUBLICATION_EVENT_REF: ${{ github.event.ref }}", stable)
        self.assertIn("ref: ${{ inputs.source_sha || github.sha }}", stable)
        self.assertIn("source_sha:", stable)
        self.assertIn("native_run_id:", stable)
        self.assertIn("current main", stable)
        self.assertIn("actions/workflows/build.yml/runs?branch=main&head_sha=$SOURCE_SHA&event=push&per_page=100", stable)
        self.assertIn("run.get('head_sha') == os.environ['SOURCE_SHA']", stable)
        self.assertIn("run.get('event') == 'push'", stable)
        self.assertIn("run.get('path') == '.github/workflows/build.yml'", stable)
        self.assertIn("allowed_run_state(run)", stable)
        self.assertIn("required_jobs_pass(candidate_id)", stable)
        self.assertNotIn("run.get('conclusion') == 'success'", stable)
        self.assertIn("matches[0].get('status') == 'completed'", stable)
        self.assertIn("--stable", stable)
        self.assertIn('--draft --latest=false --notes-file "docs/releases/$tag.md"', stable)
        self.assertIn("isPrerelease", stable)
        self.assertIn("--draft=false --latest", stable)
        release_check = stable.index('if gh release view "$tag"')
        tag_check = stable.index('if gh api "repos/$GITHUB_REPOSITORY/git/ref/tags/$tag"')
        create_release = stable.index('gh release create "$tag"')
        self.assertLess(release_check, create_release)
        self.assertLess(tag_check, create_release)
        self.assertIn("refusing to move it.", stable)
        stable_guard = 'if [[ "$version" =~ ^[0-9]+\\.[0-9]+\\.[0-9]+$ ]]'
        rc_guard = '[[ "$version" =~ ^[0-9]+\\.[0-9]+\\.[0-9]+-rc\\.[0-9]+$ ]]'
        self.assertIn(stable_guard, rc)
        self.assertIn(rc_guard, rc)
        self.assertLess(rc.index(stable_guard), rc.index(rc_guard))
        self.assertIn("publish=false", rc)

    def _make_repo(self, repo: Path, *, bom: bool = False, version: str = VERSION,
                   scope: dict | None = None) -> None:
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
        (repo / f"docs/releases/v{version}.md").write_text("Synthetic stable notes.\n", encoding="utf-8")
        (repo / "VERSION").write_text(version + "\n", encoding="utf-8")
        scope = dict(scope or SCOPE)
        scope.setdefault("target_version", version)
        acceptance = {"version": version, "channel": "stable", "stable_ready": True,
                      "personal_catalogs_in_release": False, "known_open_gates": [],
                      "publication_requires_successful_jobs": ["test", "windows"],
                      "release_scope": scope}
        (repo / "release/ACCEPTANCE.json").write_text(json.dumps(acceptance),
                                                       encoding="utf-8-sig" if bom else "utf-8")
        run = {"id": RUN_ID, "head_sha": SHA, "head_branch": "main", "status": "completed",
               "conclusion": "success",
               "event": "push", "path": ".github/workflows/build.yml",
               "head_repository": {"full_name": REPOSITORY}}
        (repo / "verified-run.json").write_text(json.dumps(run), encoding="utf-8")
        (repo / "verified-jobs.json").write_text(json.dumps({"jobs": [
            {"name": "test", "status": "completed", "conclusion": "success"},
            {"name": "windows", "status": "completed", "conclusion": "success"},
            {"name": "macos (macos-15)", "status": "completed", "conclusion": "cancelled"},
        ]}), encoding="utf-8")
        incoming = repo / "incoming/WhereIsItAI-egui-windows-x64-unsigned-candidate"
        incoming.mkdir(parents=True)
        installer_name = f"WhereIsItAI-egui-{version}-windows-x64-setup.exe"
        installer = incoming / installer_name
        installer.write_bytes(b"synthetic setup bytes")
        checksum = f"{hashlib.sha256(installer.read_bytes()).hexdigest()}  {installer_name}\n"
        (incoming / "SHA256SUMS.txt").write_text(checksum, encoding="utf-8-sig" if bom else "ascii")
        product = {"schema": "whereisit-egui-product-candidate-v1", "channel": "stable",
                   "target_ui": "Rust/egui", "platform": "windows", "architecture": "x64",
                   "version": version, "commit": SHA, "run_id": RUN_ID,
                   "installer_sha256": hashlib.sha256(installer.read_bytes()).hexdigest(),
                   "signing": "unsigned", "stable_ready": True,
                   "personal_catalogs_included": False, "release_scope": scope}
        (incoming / "provenance.json").write_text(json.dumps(product), encoding="utf-8-sig" if bom else "utf-8")

    @staticmethod
    def _prepare(repo: Path) -> subprocess.CompletedProcess[str]:
        env = {**os.environ, "SOURCE_SHA": SHA, "CI_RUN_ID": RUN_ID,
               "GITHUB_REPOSITORY": REPOSITORY}
        return subprocess.run([sys.executable, str(repo / "tools/prepare_release_assets.py"), "--stable"],
                              cwd=repo, env=env, capture_output=True, text=True, check=False)


if __name__ == "__main__":
    unittest.main()
