import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "candidate_publication_allowed",
    ROOT / "tools/dev/candidate_publication_allowed.py",
)
POLICY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(POLICY)


VERSION = "1.1.0-rc.1"
SCOPE = {"target_version": VERSION, "ship_platforms": ["windows-x64"]}


def manifests():
    acceptance = {
        "version": VERSION,
        "channel": "prerelease",
        "stable_ready": False,
        "candidate_publication_approved": True,
        "release_scope": SCOPE,
    }
    product = {
        "schema": "whereisit-egui-product-candidate-v1",
        "version": VERSION,
        "channel": "prerelease",
        "stable_ready": False,
        "candidate_publication_approved": True,
        "platform": "windows",
        "architecture": "x64",
        "release_scope": SCOPE,
    }
    return acceptance, product


class CandidatePublicationPolicyTests(unittest.TestCase):
    def test_allows_matching_explicitly_approved_windows_rc(self):
        acceptance, product = manifests()
        self.assertIsNone(POLICY.policy_error(VERSION, acceptance, product))

    def test_missing_approval_fails_closed(self):
        acceptance, product = manifests()
        del acceptance["candidate_publication_approved"]
        self.assertIsNotNone(POLICY.policy_error(VERSION, acceptance, product))

    def test_false_approval_fails_closed(self):
        acceptance, product = manifests()
        acceptance["candidate_publication_approved"] = False
        product["candidate_publication_approved"] = False
        self.assertIsNotNone(POLICY.policy_error(VERSION, acceptance, product))

    def test_string_true_approval_fails_closed(self):
        acceptance, product = manifests()
        acceptance["candidate_publication_approved"] = "true"
        product["candidate_publication_approved"] = "true"
        self.assertIsNotNone(POLICY.policy_error(VERSION, acceptance, product))

    def test_release_scope_cannot_approve_a_different_version(self):
        acceptance, product = manifests()
        acceptance['release_scope'] = {**SCOPE, 'target_version': '1.0.0'}
        self.assertIsNotNone(POLICY.policy_error(VERSION, acceptance, product))

    def test_workflow_short_circuit_accepts_only_json_boolean_true(self):
        workflow = (ROOT / '.github/workflows/publish-validated-candidate.yml').read_text()
        line = next(line for line in workflow.splitlines() if line.strip().startswith('approval='))
        command = line.split("python3 -c '", 1)[1].rsplit("')\"", 1)[0]
        for value in [True, False, None, 'true', 'True', 1]:
            with self.subTest(value=value), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / 'release').mkdir()
                (root / 'release/ACCEPTANCE.json').write_text(
                    json.dumps({'candidate_publication_approved': value}), encoding='utf-8')
                result = subprocess.run([sys.executable, '-c', command], cwd=root,
                                        capture_output=True, text=True, check=True)
                self.assertEqual(result.stdout.strip(), 'True' if value is True else 'False')

    def test_acceptance_or_product_version_mismatch_fails_closed(self):
        acceptance, product = manifests()
        acceptance["version"] = "1.1.0-rc.2"
        self.assertIsNotNone(POLICY.policy_error(VERSION, acceptance, product))
        acceptance, product = manifests()
        product["version"] = "1.1.0-rc.2"
        self.assertIsNotNone(POLICY.policy_error(VERSION, acceptance, product))

    def test_stable_and_non_rc_versions_fail_closed(self):
        acceptance, product = manifests()
        acceptance["version"] = "1.1.0"
        acceptance["channel"] = "stable"
        product["version"] = "1.1.0"
        product["channel"] = "stable"
        self.assertIsNotNone(POLICY.policy_error("1.1.0", acceptance, product))
        acceptance, product = manifests()
        self.assertIsNotNone(POLICY.policy_error("1.1.0", acceptance, product))

    def test_candidate_publication_guard_precedes_release_creation(self):
        workflow = (ROOT / ".github/workflows/publish-validated-candidate.yml").read_text()
        approval_short_circuit = workflow.index('approval="$(python3 -c')
        release_view = workflow.index('gh release view "v$version"')
        download = workflow.index("gh run download")
        guard = workflow.index("candidate_publication_allowed.py")
        create = workflow.index('gh release create "$tag"')
        self.assertLess(approval_short_circuit, release_view)
        self.assertLess(approval_short_circuit, download)
        self.assertLess(guard, create)
        self.assertIn("Stable releases use the separate manual Windows stable publisher.", workflow)

    def test_macos_packaging_is_gated_by_shipped_platform_scope(self):
        workflow = (ROOT / ".github/workflows/build.yml").read_text()
        selector = workflow.index("Select explicitly shipped macOS product packaging scope")
        compile_step = workflow.index("Compile standard Rust/egui product binary", selector)
        swift_build = workflow.index("bash apps/macos/build.sh", selector)
        package_step = workflow.index("bash installers/macos/package-egui-product.sh", selector)
        self.assertLess(selector, compile_step)
        self.assertLess(compile_step, swift_build)
        self.assertIn("macos-arm64", workflow[selector:package_step])
        self.assertIn("if: steps.release.outputs.is_rc == 'true'", workflow[package_step - 120:package_step])
        mac_job = workflow[selector:]
        self.assertIn("- run: cargo test --workspace --locked", mac_job)
        self.assertIn("- run: bash apps/macos/build.sh", mac_job)


if __name__ == "__main__":
    unittest.main()
