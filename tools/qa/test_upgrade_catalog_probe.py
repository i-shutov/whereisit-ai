"""Require the exact installed versions and checkpoint for each upgrade pair."""
import unittest
from contextlib import closing
import sqlite3
import tempfile
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch
from upgrade_catalog_probe import (FORMAT, RC1, RC2, RC3, RC4, STABLE, NEXT_RC1, NEXT_STABLE,
    HOTFIX_STABLE, MEMORY_FIX_STABLE, preferences_payload, preferences_snapshot, validate_user_store_pair,
    verify_user_store_checkpoint, sha256,
    ENTRY_NOTE, ENTRY_TAGS, expected_cli_version, validate_checkpoint_versions,
    seed_fixture_annotation, require_fixture_annotation)


class InstalledVersionContract(unittest.TestCase):
    def test_fixture_annotation_runs_unicode_trigger_and_is_visible(self):
        with tempfile.TemporaryDirectory() as directory:
            db = Path(directory) / "generated.sqlite"
            with closing(sqlite3.connect(db)) as connection, connection:
                connection.executescript("""
                    CREATE TABLE payload(id INTEGER PRIMARY KEY, relative_path TEXT,
                        description TEXT, tags TEXT, normalized_note TEXT);
                    CREATE VIEW entries AS SELECT * FROM payload;
                    CREATE TRIGGER normalize_note INSTEAD OF UPDATE OF description,tags ON entries
                    BEGIN UPDATE payload SET description=new.description,tags=new.tags,
                        normalized_note=unicode_lower(new.description)
                        WHERE id=new.id; END;
                """)
                connection.execute("INSERT INTO payload VALUES(1,?, '', '', '')",
                                   ("Музыка/track-a.mp3",))
            seed_fixture_annotation(db)
            with closing(sqlite3.connect(db)) as connection:
                row = connection.execute("SELECT id,description,tags,normalized_note FROM entries").fetchone()
            self.assertEqual(row, (1, ENTRY_NOTE, ENTRY_TAGS, ENTRY_NOTE.lower()))
            require_fixture_annotation({"entry_notes": [list(row[:3])]})

    def test_missing_fixture_annotation_is_refused(self):
        with self.assertRaisesRegex(RuntimeError, "synthetic_note_not_visible"):
            require_fixture_annotation({"entry_notes": [[1, "", ENTRY_TAGS]]})

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

    def test_stable_pair_is_only_rc4_to_100(self):
        self.assertEqual(expected_cli_version("prepare", RC4, STABLE), "WhereIsIt AI " + RC4)
        self.assertEqual(expected_cli_version("verify", RC4, STABLE), "WhereIsIt AI " + STABLE)
        for source, target in [(RC3, STABLE), (RC4, RC4), (STABLE, RC4), (RC2, STABLE)]:
            with self.subTest(source=source, target=target):
                with self.assertRaisesRegex(RuntimeError, "unsupported_upgrade_version_pair"):
                    expected_cli_version("verify", source, target)

    def test_11_candidate_pair_uses_pinned_stable_predecessor(self):
        self.assertEqual(expected_cli_version("prepare", STABLE, NEXT_RC1), "WhereIsIt AI " + STABLE)
        self.assertEqual(expected_cli_version("verify", STABLE, NEXT_RC1), "WhereIsIt AI " + NEXT_RC1)
        for source, target in [(RC4, NEXT_RC1), (STABLE, STABLE), (NEXT_RC1, STABLE)]:
            with self.subTest(source=source, target=target):
                with self.assertRaisesRegex(RuntimeError, "unsupported_upgrade_version_pair"):
                    expected_cli_version("verify", source, target)

    def test_11_stable_pair_uses_pinned_100_predecessor(self):
        self.assertEqual(expected_cli_version("prepare", STABLE, NEXT_STABLE), "WhereIsIt AI " + STABLE)
        self.assertEqual(expected_cli_version("verify", STABLE, NEXT_STABLE), "WhereIsIt AI " + NEXT_STABLE)
        for source, target in [(RC4, NEXT_STABLE), (STABLE, STABLE), (NEXT_STABLE, STABLE)]:
            with self.subTest(source=source, target=target):
                with self.assertRaisesRegex(RuntimeError, "unsupported_upgrade_version_pair"):
                    expected_cli_version("verify", source, target)

    def test_111_hotfix_pair_uses_110_predecessor(self):
        self.assertEqual(expected_cli_version("prepare", NEXT_STABLE, HOTFIX_STABLE),
                         "WhereIsIt AI " + NEXT_STABLE)
        self.assertEqual(expected_cli_version("verify", NEXT_STABLE, HOTFIX_STABLE),
                         "WhereIsIt AI " + HOTFIX_STABLE)
        for source, target in [(STABLE, HOTFIX_STABLE), (NEXT_STABLE, NEXT_STABLE),
                               (HOTFIX_STABLE, NEXT_STABLE)]:
            with self.subTest(source=source, target=target):
                with self.assertRaisesRegex(RuntimeError, "unsupported_upgrade_version_pair"):
                    expected_cli_version("verify", source, target)

    def test_111_hotfix_requires_real_user_store_only_for_that_pair(self):
        store = Path("synthetic-user-store.sqlite")
        validate_user_store_pair(NEXT_STABLE, HOTFIX_STABLE, store)
        with self.assertRaisesRegex(RuntimeError, "user_store_required_for_hotfix"):
            validate_user_store_pair(NEXT_STABLE, HOTFIX_STABLE, None)
        with self.assertRaisesRegex(RuntimeError, "unexpected_user_store_for_upgrade_pair"):
            validate_user_store_pair(STABLE, NEXT_STABLE, store)

    def test_112_memory_fix_pair_requires_real_user_store(self):
        store = Path("synthetic-user-store.sqlite")
        self.assertEqual(expected_cli_version("prepare", HOTFIX_STABLE, MEMORY_FIX_STABLE),
                         "WhereIsIt AI " + HOTFIX_STABLE)
        self.assertEqual(expected_cli_version("verify", HOTFIX_STABLE, MEMORY_FIX_STABLE),
                         "WhereIsIt AI " + MEMORY_FIX_STABLE)
        validate_user_store_pair(HOTFIX_STABLE, MEMORY_FIX_STABLE, store)
        with self.assertRaisesRegex(RuntimeError, "user_store_required_for_memory_fix"):
            validate_user_store_pair(HOTFIX_STABLE, MEMORY_FIX_STABLE, None)
        with self.assertRaisesRegex(RuntimeError, "unexpected_user_store_for_upgrade_pair"):
            validate_user_store_pair(NEXT_STABLE, MEMORY_FIX_STABLE, store)

    def test_112_preferences_use_real_app_store_and_checkpoint_note(self):
        db = Path("synthetic-catalog.sqlite")
        store = Path("synthetic-user-store.sqlite")
        note = "Synthetic shelf preferences — 1.1.1 to 1.1.2"
        expected = {
            "volume_id": 4, "display_name": "Synthetic Upgrade Disk",
            "shelf_label": "QA-HOTFIX-111", "model": "Synthetic Model",
            "filesystem": "NTFS", "capacity_bytes": 123456,
            "note": note, "revision": 1,
        }
        saved = {"saved": True, "volume_id": 4, "revision": 1}
        with patch("upgrade_catalog_probe.call_cli", side_effect=[saved, expected]) as call:
            self.assertEqual(preferences_snapshot(Path("atlas.exe"), db, store, 4,
                                                  create=True, note=note), expected)
        save_request = call.call_args_list[0].args[1]
        self.assertEqual(save_request["op"], "disk_preferences_set")
        self.assertEqual(save_request["note"], note)
        self.assertEqual(call.call_args_list[1].args[1]["op"], "disk_preferences_get")

    def test_112_checkpoint_binds_exact_pair_and_checks_app_preferences(self):
        source, target = HOTFIX_STABLE, MEMORY_FIX_STABLE
        checkpoint = {"format": FORMAT, "source_release": source, "target_release": target}
        validate_checkpoint_versions(checkpoint, source, target)
        for wrong_source, wrong_target in [(NEXT_STABLE, target), (source, HOTFIX_STABLE)]:
            with self.subTest(source=wrong_source, target=wrong_target):
                with self.assertRaisesRegex(RuntimeError, "wrong_checkpoint"):
                    validate_checkpoint_versions(checkpoint, wrong_source, wrong_target)

        note = f"Synthetic shelf preferences — {source} to {target}"
        preferences = {"volume_id": 4, "display_name": "Synthetic Upgrade Disk",
                       "shelf_label": "QA-HOTFIX-111", "model": "Synthetic Model",
                       "filesystem": "NTFS", "capacity_bytes": 123456,
                       "note": note, "revision": 1}
        with TemporaryDirectory() as directory:
            store = Path(directory) / "generated-sidecar.sqlite"
            store.write_bytes(b"synthetic application preferences sqlite fixture")
            user_checkpoint = {"path": str(store), "sha256_after_prepare": sha256(store),
                               "preferences": preferences}
            with patch("upgrade_catalog_probe.preferences_snapshot", return_value=preferences) as read:
                self.assertTrue(verify_user_store_checkpoint(
                    Path("atlas.exe"), Path("catalog.sqlite"), store, user_checkpoint, note=note))
            read.assert_called_once_with(Path("atlas.exe"), Path("catalog.sqlite"),
                                         store, 4, create=False, note=note)

    def test_111_preferences_prepare_and_verify_use_cli_contract(self):
        db = Path("synthetic-catalog.sqlite")
        store = Path("synthetic-user-store.sqlite")
        expected = {
            "volume_id": 4, "display_name": "Synthetic Upgrade Disk",
            "shelf_label": "QA-HOTFIX-111", "model": "Synthetic Model",
            "filesystem": "NTFS", "capacity_bytes": 123456,
            "note": "Synthetic shelf preferences — 1.1.0 to 1.1.1", "revision": 1,
        }
        saved = {"saved": True, "volume_id": 4, "revision": 1}
        with patch("upgrade_catalog_probe.call_cli", side_effect=[saved, expected]) as call:
            self.assertEqual(preferences_snapshot(Path("atlas.exe"), db, store, 4, create=True), expected)
        save_request, get_request = (args.args[1] for args in call.call_args_list)
        self.assertEqual(save_request, preferences_payload(db, store, 4))
        self.assertEqual(save_request["op"], "disk_preferences_set")
        self.assertEqual(save_request["expected_revision"], 0)
        self.assertEqual(save_request["confirmed"], True)
        self.assertEqual(get_request["op"], "disk_preferences_get")
        with patch("upgrade_catalog_probe.call_cli", return_value=expected) as call:
            self.assertEqual(preferences_snapshot(Path("atlas.exe"), db, store, 4, create=False), expected)
        self.assertEqual(call.call_count, 1)
        self.assertEqual(call.call_args.args[1]["op"], "disk_preferences_get")

    def test_111_mismatched_preferences_fail_closed_on_verify(self):
        wrong = {"volume_id": 4, "display_name": "Changed", "revision": 1}
        with patch("upgrade_catalog_probe.call_cli", return_value=wrong):
            with self.assertRaisesRegex(RuntimeError, "synthetic_user_store_preferences_mismatch"):
                preferences_snapshot(Path("atlas.exe"), Path("c.sqlite"),
                                     Path("u.sqlite"), 4, create=False)

    def test_111_verify_checks_checkpoint_path_bytes_and_semantics(self):
        expected = {
            "volume_id": 4, "display_name": "Synthetic Upgrade Disk",
            "shelf_label": "QA-HOTFIX-111", "model": "Synthetic Model",
            "filesystem": "NTFS", "capacity_bytes": 123456,
            "note": "Synthetic shelf preferences — 1.1.0 to 1.1.1", "revision": 1,
        }
        with TemporaryDirectory() as directory:
            store = Path(directory) / "generated-sidecar.sqlite"
            store.write_bytes(b"synthetic preferences sidecar")
            checkpoint = {"path": str(store), "sha256_after_prepare": sha256(store),
                          "preferences": expected}
            with patch("upgrade_catalog_probe.preferences_snapshot", return_value=expected) as read:
                self.assertTrue(verify_user_store_checkpoint(
                    Path("atlas.exe"), Path("catalog.sqlite"), store, checkpoint))
            read.assert_called_once_with(Path("atlas.exe"), Path("catalog.sqlite"),
                                         store, 4, create=False,
                                         note="Synthetic shelf preferences — 1.1.0 to 1.1.1")

            changed = dict(checkpoint, sha256_after_prepare="0" * 64)
            with self.assertRaisesRegex(RuntimeError, "user_store_bytes_changed_by_upgrade"):
                verify_user_store_checkpoint(Path("atlas.exe"), Path("catalog.sqlite"),
                                             store, changed)
            with self.assertRaisesRegex(RuntimeError, "user_store_path_changed"):
                verify_user_store_checkpoint(Path("atlas.exe"), Path("catalog.sqlite"),
                                             Path(directory) / "other.sqlite", checkpoint)

    def test_workflow_and_lifecycle_pin_11_stable_to_100(self):
        root = Path(__file__).resolve().parents[2]
        workflow = (root / ".github/workflows/upgrade-rc4-stable-windows.yml").read_text(encoding="utf-8")
        lifecycle = (root / "tools/qa/upgrade-rc4-stable-windows.ps1").read_text(encoding="utf-8")
        pin = "938fece2d74793c2eda7a1b84082ab5db3f8a092"
        self.assertIn("'1.1.0' { $sourceSha = $env:V100_SHA; $sourceVersion = '1.0.0' }", workflow)
        self.assertIn("'1.1.0' { @{ source = '1.0.0'; sha = '" + pin + "' } }", lifecycle)
        self.assertIn("V100_SHA: " + pin, workflow)
        self.assertIn("$env:TARGET_SHA", lifecycle)

    def test_stable_checkpoint_is_exact_rc4_to_100_pair(self):
        checkpoint = {"format": FORMAT, "source_release": RC4, "target_release": STABLE}
        validate_checkpoint_versions(checkpoint, RC4, STABLE)
        for source, target in [(RC3, STABLE), (RC4, RC4), (STABLE, RC4)]:
            with self.subTest(source=source, target=target):
                with self.assertRaisesRegex(RuntimeError, "wrong_checkpoint"):
                    validate_checkpoint_versions(checkpoint, source, target)

    def test_11_candidate_checkpoint_is_exact_100_to_11rc1_pair(self):
        checkpoint = {"format": FORMAT, "source_release": STABLE, "target_release": NEXT_RC1}
        validate_checkpoint_versions(checkpoint, STABLE, NEXT_RC1)
        for source, target in [(RC4, NEXT_RC1), (STABLE, STABLE), (NEXT_RC1, STABLE)]:
            with self.subTest(source=source, target=target):
                with self.assertRaisesRegex(RuntimeError, "wrong_checkpoint"):
                    validate_checkpoint_versions(checkpoint, source, target)

    def test_11_stable_checkpoint_is_exact_100_to_110_pair(self):
        checkpoint = {"format": FORMAT, "source_release": STABLE, "target_release": NEXT_STABLE}
        validate_checkpoint_versions(checkpoint, STABLE, NEXT_STABLE)
        for source, target in [(RC4, NEXT_STABLE), (STABLE, NEXT_RC1), (NEXT_STABLE, STABLE)]:
            with self.subTest(source=source, target=target):
                with self.assertRaisesRegex(RuntimeError, "wrong_checkpoint"):
                    validate_checkpoint_versions(checkpoint, source, target)

    def test_111_hotfix_checkpoint_is_exact_110_to_111_pair(self):
        checkpoint = {"format": FORMAT, "source_release": NEXT_STABLE,
                      "target_release": HOTFIX_STABLE}
        validate_checkpoint_versions(checkpoint, NEXT_STABLE, HOTFIX_STABLE)
        for source, target in [(STABLE, HOTFIX_STABLE), (NEXT_STABLE, NEXT_RC1),
                               (HOTFIX_STABLE, NEXT_STABLE)]:
            with self.subTest(source=source, target=target):
                with self.assertRaisesRegex(RuntimeError, "wrong_checkpoint"):
                    validate_checkpoint_versions(checkpoint, source, target)

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
