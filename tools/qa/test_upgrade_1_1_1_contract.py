"""Contract for exact-source installed Windows upgrade pairs."""
from pathlib import Path
import re
import sys
import unittest


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/upgrade-rc4-stable-windows.yml"
LIFECYCLE = ROOT / "tools/qa/upgrade-rc4-stable-windows.ps1"
sys.path.insert(0, str(ROOT / "tools/qa"))
from upgrade_catalog_probe import (FORMAT, HOTFIX_STABLE, NEXT_STABLE,
                                   expected_cli_version, validate_checkpoint_versions)


def workflow_pairs(text):
    env_pins = dict(re.findall(r"^\s+(\w+_SHA): ([0-9a-f]{40})$", text, re.M))
    rows = re.findall(
        r"^\s*'([^']+)' \{ \$sourceSha = \$env:(\w+); \$sourceVersion = '([^']+)' \}$",
        text,
        re.M,
    )
    return {target: (version, env_pins[pin_name]) for target, pin_name, version in rows}


def lifecycle_pairs(text):
    rows = re.findall(
        r"^\s*'([^']+)' \{ @\{ source = '([^']+)'; sha = '([0-9a-f]{40})' \} \}$",
        text,
        re.M,
    )
    return {target: (source, sha) for target, source, sha in rows}


class WindowsUpgradeContract(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.workflow = WORKFLOW.read_text(encoding="utf-8")
        cls.lifecycle = LIFECYCLE.read_text(encoding="utf-8")

    def test_workflow_and_lifecycle_exact_pairs_match(self):
        expected = {
            "1.0.0": ("1.0.0-rc.4", "751fa5454c813e60b61800b517685ccab7e9776a"),
            "1.1.0-rc.1": ("1.0.0", "938fece2d74793c2eda7a1b84082ab5db3f8a092"),
            "1.1.0": ("1.0.0", "938fece2d74793c2eda7a1b84082ab5db3f8a092"),
            "1.1.1": ("1.1.0", "7f684f47aab42fb211dfa0af1085ace311ef9f8d"),
        }
        self.assertEqual(workflow_pairs(self.workflow), expected)
        self.assertEqual(lifecycle_pairs(self.lifecycle), expected)
        for target, (source, _pin) in expected.items():
            with self.subTest(source=source, target=target):
                self.assertEqual(expected_cli_version("prepare", source, target),
                                 "WhereIsIt AI " + source)
                self.assertEqual(expected_cli_version("verify", source, target),
                                 "WhereIsIt AI " + target)
                validate_checkpoint_versions(
                    {"format": FORMAT, "source_release": source, "target_release": target},
                    source,
                    target,
                )
        with self.assertRaisesRegex(RuntimeError, "unsupported_upgrade_version_pair"):
            expected_cli_version("verify", NEXT_STABLE, HOTFIX_STABLE + ".0")

    def test_lifecycle_is_hosted_runner_only_and_exact_target_bound(self):
        for gate in (
            "$env:GITHUB_ACTIONS -ne 'true'",
            "$env:RUNNER_ENVIRONMENT -ne 'github-hosted'",
            "-not $env:RUNNER_TEMP",
            "$targetSha -ne $env:TARGET_SHA",
            "$sourceSha -ne $pair.sha",
            "default { throw \"Unsupported target release",
        ):
            self.assertIn(gate, self.lifecycle)
        self.assertIn("git -C source rev-parse HEAD", self.workflow)
        self.assertIn("git -C source rev-parse HEAD).Trim() -ne $expectedSourceSha", self.workflow)

    def test_generated_external_catalog_ctf_and_sidecar_are_checked(self):
        self.assertIn("$work = Join-Path $env:RUNNER_TEMP", self.lifecycle)
        self.assertIn("$db = Join-Path $work 'external.sqlite'", self.lifecycle)
        self.assertIn("$fixture = Join-Path $work 'fixture'", self.lifecycle)
        self.assertIn("$sidecar = Join-Path $work 'generated-user-sidecar.sqlite'", self.lifecycle)
        self.assertIn("$placeholder = Join-Path $fixture 'external-user-ctf-placeholder.CTF'", self.lifecycle)
        self.assertIn("(File-Sha $db) -ne $CatalogSha", self.lifecycle)
        self.assertIn("(File-Sha $placeholder) -ne $CtfSha", self.lifecycle)
        self.assertIn("(File-Sha $sidecar) -ne $script:sidecarSha", self.lifecycle)
        self.assertIn("external_sidecar_preserved = (File-Sha $sidecar) -eq $sidecarSha", self.lifecycle)
        self.assertIn("Verify-External $beforeRollback $ctfSha", self.lifecycle)
        self.assertIn("synthetic_only = $true", self.lifecycle)
        self.assertIn("if ($targetVersion -eq '1.1.1') { $probeArgs += @('--user-store', $sidecar) }", self.lifecycle)
        self.assertIn("if ($targetVersion -ne '1.1.1')", self.lifecycle)
        self.assertIn("preferences_snapshot(", (ROOT / "tools/qa/upgrade_catalog_probe.py").read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
