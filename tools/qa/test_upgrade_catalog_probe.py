"""Require the exact installed versions and checkpoint for each upgrade pair."""
import unittest
from pathlib import Path
from upgrade_catalog_probe import (FORMAT, RC1, RC2, RC3, RC4,
    expected_cli_version, validate_checkpoint_versions)


class InstalledVersionContract(unittest.TestCase):
    def test_os_upgrade_qa_uses_pinned_sqlite_metadata(self):
        root = Path(__file__).resolve().parents[2]
        for path in (
            root / "tools/qa/upgrade-rc3-macos.sh",
            root / "tools/qa/upgrade-rc3-windows.ps1",
        ):
            contents = path.read_text(encoding="utf-8")
            self.assertIn("refresh_sqlite_notices.py", contents)
            self.assertNotIn("3.51.3", contents)

    def test_installer_smokes_follow_pinned_sqlite_version(self):
        root = Path(__file__).resolve().parents[2]
        mac = (root / 'installers/macos/smoke-package.sh').read_text(encoding='utf-8')
        win = (root / 'installers/windows/smoke-installer.ps1').read_text(encoding='utf-8')
        self.assertIn('refresh_sqlite_notices.py', mac)
        self.assertIn('refresh_sqlite_notices.py', win)
        self.assertIn('[regex]::Match', win)
        self.assertNotIn('$Matches[1]', win)
        self.assertIn('SQLite-${EXPECTED_SQLITE}', mac)
        self.assertIn('SQLite-$expectedSqlite', win)
        self.assertNotIn('3.51.3', mac)
        self.assertNotIn('3.51.3', win)

    def test_historical_pair_remains_exact(self):
        self.assertEqual(expected_cli_version("prepare", RC1, RC2), "WhereIsIt AI " + RC1)
        self.assertEqual(expected_cli_version("verify", RC1, RC2), "WhereIsIt AI " + RC2)

    def test_current_pair_uses_actual_predecessor(self):
        for source, target in [(RC2, RC3), (RC3, RC4)]:
            with self.subTest(source=source, target=target):
                self.assertEqual(expected_cli_version("prepare", source, target), "WhereIsIt AI " + source)
                self.assertEqual(expected_cli_version("verify", source, target), "WhereIsIt AI " + target)

    def test_rc4_checkpoint_requires_rc3_to_rc4_pair(self):
        checkpoint = {"format": FORMAT, "source_release": RC3, "target_release": RC4}
        validate_checkpoint_versions(checkpoint, RC3, RC4)
        for source, target in [(RC2, RC3), (RC2, RC4), (RC4, RC3)]:
            with self.subTest(source=source, target=target):
                with self.assertRaisesRegex(RuntimeError, "wrong_checkpoint"):
                    validate_checkpoint_versions(checkpoint, source, target)

    def test_checkpoint_cannot_be_reused_for_another_upgrade(self):
        checkpoint = {"format": FORMAT, "source_release": RC2, "target_release": "1.0.0-rc.3"}
        validate_checkpoint_versions(checkpoint, RC2, "1.0.0-rc.3")
        for source, target in [(RC1, RC2), (RC2, "1.0.0-rc.4")]:
            with self.assertRaisesRegex(RuntimeError, "wrong_checkpoint"):
                validate_checkpoint_versions(checkpoint, source, target)
        with self.assertRaisesRegex(RuntimeError, "wrong_checkpoint"):
            validate_checkpoint_versions({"format": FORMAT, "source_release": RC2}, RC2, "1.0.0-rc.3")

    def test_unknown_pairs_and_modes_fail_closed(self):
        for mode, source, target in [
            ("prepare", RC2, RC2),
            ("verify", RC2, "1.0.0"),
            ("verify", RC2, RC4),
            ("verify", RC4, RC3),
            ("invalid", RC1, RC2),
        ]:
            with self.assertRaisesRegex(RuntimeError, "unsupported_upgrade_version_pair"):
                expected_cli_version(mode, source, target)


if __name__ == "__main__":
    unittest.main()
