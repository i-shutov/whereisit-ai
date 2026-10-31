"""Regression: an RC2-to-RC3 run must never validate RC1-to-RC2 versions."""
import unittest
from upgrade_catalog_probe import (FORMAT, RC1, RC2,
    expected_cli_version, validate_checkpoint_versions)


class InstalledVersionContract(unittest.TestCase):
    def test_historical_pair_remains_exact(self):
        self.assertEqual(expected_cli_version("prepare", RC1, RC2), "WhereIsIt AI " + RC1)
        self.assertEqual(expected_cli_version("verify", RC1, RC2), "WhereIsIt AI " + RC2)

    def test_current_pair_uses_actual_predecessor(self):
        self.assertEqual(expected_cli_version("prepare", RC2, "1.0.0-rc.3"), "WhereIsIt AI " + RC2)
        self.assertEqual(expected_cli_version("verify", RC2, "1.0.0-rc.3"), "WhereIsIt AI 1.0.0-rc.3")

    def test_checkpoint_cannot_be_reused_for_another_upgrade(self):
        checkpoint = {"format": FORMAT, "source_release": RC2, "target_release": "1.0.0-rc.3"}
        validate_checkpoint_versions(checkpoint, RC2, "1.0.0-rc.3")
        for source, target in [(RC1, RC2), (RC2, "1.0.0-rc.4")]:
            with self.assertRaisesRegex(RuntimeError, "wrong_checkpoint"):
                validate_checkpoint_versions(checkpoint, source, target)
        with self.assertRaisesRegex(RuntimeError, "wrong_checkpoint"):
            validate_checkpoint_versions({"format": FORMAT, "source_release": RC2}, RC2, "1.0.0-rc.3")

    def test_unknown_pairs_and_modes_fail_closed(self):
        for mode, source, target in [("prepare", RC2, RC2), ("verify", RC2, "1.0.0"), ("invalid", RC1, RC2)]:
            with self.assertRaisesRegex(RuntimeError, "unsupported_upgrade_version_pair"):
                expected_cli_version(mode, source, target)


if __name__ == "__main__":
    unittest.main()
