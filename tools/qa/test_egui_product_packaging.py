"""Synthetic contracts for the side-by-side egui product release payload."""
from __future__ import annotations

import hashlib
import json
import os
import plistlib
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
PREPARER = ROOT / "tools" / "prepare_release_assets.py"
CHECKER = ROOT / "tools" / "qa" / "check_public_evidence.py"
VERSION = "1.0.0-rc.9"
SHA = "0123456789abcdef" * 2 + "01234567"
RUN_ID = "123456789"
REPOSITORY = "example/whereisit"


class EguiProductPackagingContract(unittest.TestCase):
    def test_product_installer_contract_is_separate_from_legacy_and_qa(self):
        iss = (ROOT / "installers/windows/WhereIsItAI-EguiWin64.iss").read_text(encoding="utf-8")
        build = (ROOT / "installers/windows/build-egui-product.ps1").read_text(encoding="utf-8")
        smoke = (ROOT / "installers/windows/smoke-egui-product.ps1").read_text(encoding="utf-8")
        legacy = (ROOT / "installers/windows/WhereIsItAI.iss").read_text(encoding="utf-8")
        qa = (ROOT / "installers/windows/WhereIsItAI-EguiWin64QA.iss").read_text(encoding="utf-8")
        actions = (ROOT / ".github/workflows/build.yml").read_text(encoding="utf-8")
        self.assertIn("atlas-desktop-egui.exe", iss)
        self.assertIn("PrivilegesRequired=lowest", iss)
        self.assertIn("whereisit-ai-egui.exe", iss)
        self.assertIn("licenses/*", iss)
        self.assertNotIn("postinstall", iss.lower())
        self.assertNotIn('Source: "{#BuildDir}/atlas-windows.exe"', iss)
        self.assertNotEqual(self._value(iss, "AppId"), self._value(legacy, "AppId"))
        self.assertNotEqual(self._value(iss, "AppId"), self._value(qa, "AppId"))
        self.assertNotEqual(self._value(iss, "DefaultDirName"), self._value(legacy, "DefaultDirName"))
        self.assertIn(" prerelease-only", build)
        self.assertIn("Refusing product installer overwrite", build)
        self.assertIn("external SQLite", smoke)
        self.assertIn("standard product GUI", smoke)
        self.assertIn("WhereIsItAI-egui-windows-x64-unsigned-candidate", actions)
        self.assertIn("WhereIsItAI-egui-Win64-installed-QA-unsigned", actions)
        self.assertIn("WhereIsItAI-Windows-x64-unsigned-candidate", actions)

    def test_macos_product_bundle_and_lifecycle_are_architecture_specific(self):
        package = (ROOT / "installers/macos/package-egui-product.sh").read_text(encoding="utf-8")
        smoke = (ROOT / "installers/macos/smoke-egui-product.sh").read_text(encoding="utf-8")
        info = plistlib.loads((ROOT / "apps/desktop-egui/Info.plist").read_bytes())
        components = (ROOT / "installers/macos/egui-components.plist").read_text(encoding="utf-8")
        legacy = (ROOT / "installers/macos/package.sh").read_text(encoding="utf-8")
        qa = (ROOT / "apps/desktop-egui/package-preview-macos.sh").read_text(encoding="utf-8")
        actions = (ROOT / ".github/workflows/build.yml").read_text(encoding="utf-8")
        self.assertEqual(info["CFBundleExecutable"], "WhereIsItAI")
        self.assertEqual(info["CFBundleIdentifier"], "ai.whereisit.desktop.egui")
        self.assertEqual(info["CFBundleName"], "WhereIsIt AI")
        self.assertEqual(info["LSMinimumSystemVersion"], "13.0")
        self.assertIn("WhereIsIt AI egui.app", components)
        self.assertIn('uname -m', package)
        self.assertIn('ad-hoc app; unsigned pkg/dmg; not notarized', package)
        self.assertIn('"stable_ready": false', package)
        self.assertIn("codesign --force --deep --sign -", package)
        self.assertIn("Developer ID signed and notarized", package)
        self.assertIn("generated catalog survives install/uninstall", smoke)
        self.assertIn("generated catalog survives install/uninstall", smoke)
        self.assertIn("WhereIsItAI-egui-${{ matrix.os }}-unsigned-candidate", actions)
        self.assertIn("dist/macos/WhereIsIt AI.app", legacy)
        self.assertIn('WhereIsItAI-${VERSION}-macOS-${ARCH}.pkg', legacy)
        self.assertIn("ai.whereisit.egui.preview", qa)

    def test_release_preparer_selects_verified_product_artifacts_only(self):
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)
            self._make_repo(repo, product=True, legacy=True)
            result = self._prepare(repo)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            assets = repo / "release-assets"
            names = {item.name for item in assets.iterdir()}
            expected = {
                f"WhereIsItAI-egui-{VERSION}-windows-x64-setup.exe",
                f"WhereIsItAI-egui-{VERSION}-macOS-arm64.pkg",
                f"WhereIsItAI-egui-{VERSION}-macOS-arm64.dmg",
                f"WhereIsItAI-egui-{VERSION}-macOS-x86_64.pkg",
                f"WhereIsItAI-egui-{VERSION}-macOS-x86_64.dmg",
                "ACCEPTANCE.json", "provenance.json", "SHA256SUMS.txt",
            }
            self.assertEqual(names, expected)
            provenance = json.loads((assets / "provenance.json").read_text(encoding="utf-8"))
            self.assertEqual(provenance["target_ui"], "Rust/egui")
            self.assertEqual(provenance["channel"], "prerelease")
            self.assertIs(provenance["stable_ready"], False)
            self.assertEqual(provenance["signing"]["windows_x64"], "unsigned")
            self.assertEqual(provenance["signing"]["macos_arm64"], "ad-hoc app; unsigned pkg/dmg; not notarized")

    def test_legacy_assets_cannot_be_selected_as_product_fallback(self):
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)
            self._make_repo(repo, product=False, legacy=True)
            result = self._prepare(repo)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Missing or ambiguous installer", result.stdout + result.stderr)
            self.assertFalse((repo / "release-assets").exists())

    def test_signed_candidate_needs_a_separately_reviewed_publication_contract(self):
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)
            self._make_repo(repo, product=True, legacy=False)
            manifest = next((repo / "incoming").rglob("provenance-egui-arm64.json"))
            candidate = json.loads(manifest.read_text(encoding="utf-8"))
            candidate["signing"] = "Developer ID signed and notarized"
            manifest.write_text(json.dumps(candidate) + "\n", encoding="utf-8")
            result = self._prepare(repo)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Unrecognized product signing state", result.stdout + result.stderr)
            self.assertFalse((repo / "release-assets").exists())

    def test_acceptance_stable_ready_true_blocks_prerelease_publication(self):
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)
            self._make_repo(repo, product=True, legacy=False)
            acceptance_path = repo / "release/ACCEPTANCE.json"
            acceptance = json.loads(acceptance_path.read_text(encoding="utf-8"))
            acceptance["stable_ready"] = True
            acceptance_path.write_text(json.dumps(acceptance) + "\n", encoding="utf-8")
            result = self._prepare(repo)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Prerelease publication requires stable_ready=false", result.stdout + result.stderr)
            self.assertFalse((repo / "release-assets").exists())

    def test_tampered_product_checksum_blocks_publication(self):
        with tempfile.TemporaryDirectory() as temp:
            repo = Path(temp)
            self._make_repo(repo, product=True, legacy=False)
            installer = next((repo / "incoming").rglob(f"WhereIsItAI-egui-{VERSION}-windows-x64-setup.exe"))
            installer.write_bytes(installer.read_bytes() + b"tampered")
            result = self._prepare(repo)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Product checksum mismatch", result.stdout + result.stderr)
            self.assertFalse((repo / "release-assets").exists())

    @staticmethod
    def _value(contents: str, name: str) -> str:
        prefix = name + "="
        return next(line.strip()[len(prefix):] for line in contents.splitlines()
                    if line.strip().startswith(prefix))

    def _make_repo(self, repo: Path, *, product: bool, legacy: bool) -> None:
        for directory in ("tools/qa", "docs/validation", "docs/releases", "release", "incoming"):
            (repo / directory).mkdir(parents=True, exist_ok=True)
        shutil.copyfile(CHECKER, repo / "tools/qa/check_public_evidence.py")
        shutil.copyfile(PREPARER, repo / "tools/prepare_release_assets.py")
        for report in ("QA_EXPORT.md", "STATUS_2026-10-28.md", "CTF_FORMAT.md", "VALIDATION.md"):
            (repo / "docs" / report).write_text("Synthetic release fixture.\n", encoding="utf-8")
        (repo / "docs/validation/database-package.json").write_text(
            json.dumps({"archive": "local-only-not-distributed", "entries": 1}) + "\n", encoding="utf-8")
        (repo / "docs/validation/performance.json").write_text(json.dumps({"samples": [1]}) + "\n", encoding="utf-8")
        (repo / "docs/releases" / f"v{VERSION}.md").write_text("Synthetic prerelease notes.\n", encoding="utf-8")
        (repo / "VERSION").write_text(VERSION + "\n", encoding="utf-8")
        acceptance = {
            "version": VERSION, "channel": "prerelease", "stable_ready": False,
            "known_open_gates": ["signing_pending"],
            "publication_requires_successful_jobs": ["test", "windows", "macos (macos-15)", "macos (macos-15-intel)"],
        }
        (repo / "release/ACCEPTANCE.json").write_text(json.dumps(acceptance) + "\n", encoding="utf-8")
        (repo / "verified-run.json").write_text(json.dumps({
            "head_sha": SHA, "head_branch": "main", "conclusion": "success",
            "head_repository": {"full_name": REPOSITORY},
        }), encoding="utf-8")
        (repo / "verified-jobs.json").write_text(json.dumps({"jobs": [
            {"name": name, "conclusion": "success"} for name in acceptance["publication_requires_successful_jobs"]
        ]}), encoding="utf-8")
        if legacy:
            legacy_dir = repo / "incoming/legacy"
            legacy_dir.mkdir(parents=True)
            (legacy_dir / f"WhereIsItAI-{VERSION}-windows-x64-setup.exe").write_bytes(b"legacy Windows fixture")
            for arch in ("arm64", "x86_64"):
                for suffix in ("pkg", "dmg"):
                    (legacy_dir / f"WhereIsItAI-{VERSION}-macOS-{arch}.{suffix}").write_bytes(b"legacy mac fixture")
        if not product:
            return
        self._add_candidate(repo, "windows", "x64", [f"WhereIsItAI-egui-{VERSION}-windows-x64-setup.exe"], "unsigned")
        for arch in ("arm64", "x86_64"):
            self._add_candidate(repo, f"mac-{arch}", arch, [
                f"WhereIsItAI-egui-{VERSION}-macOS-{arch}.pkg",
                f"WhereIsItAI-egui-{VERSION}-macOS-{arch}.dmg",
            ], "ad-hoc app; unsigned pkg/dmg; not notarized")

    def _add_candidate(self, repo: Path, folder: str, architecture: str, files: list[str], signing: str) -> None:
        root = repo / "incoming" / f"WhereIsItAI-egui-{folder}-unsigned-candidate"
        root.mkdir(parents=True)
        lines = []
        for filename in files:
            target = root / filename
            target.write_bytes(("synthetic package " + filename).encode())
            lines.append(f"{hashlib.sha256(target.read_bytes()).hexdigest()}  {filename}\n")
        checksum = "SHA256SUMS.txt" if architecture == "x64" else f"SHA256-egui-{architecture}.txt"
        (root / checksum).write_text("".join(lines), encoding="ascii")
        provenance = {
            "schema": "whereisit-egui-product-candidate-v1", "channel": "prerelease",
            "target_ui": "Rust/egui", "platform": "windows" if architecture == "x64" else "macos",
            "architecture": architecture, "version": VERSION, "commit": SHA,
            "run_id": RUN_ID, "signing": signing, "stable_ready": False,
            "personal_catalogs_included": False,
        }
        name = "provenance.json" if architecture == "x64" else f"provenance-egui-{architecture}.json"
        (root / name).write_text(json.dumps(provenance) + "\n", encoding="utf-8")

    @staticmethod
    def _prepare(repo: Path) -> subprocess.CompletedProcess[str]:
        env = {
            **os.environ,
            "SOURCE_SHA": SHA,
            "CI_RUN_ID": RUN_ID,
            "GITHUB_REPOSITORY": REPOSITORY,
        }
        return subprocess.run(
            [sys.executable, str(repo / "tools/prepare_release_assets.py")],
            cwd=repo, env=env, capture_output=True, text=True, check=False,
        )


if __name__ == "__main__":
    unittest.main()
