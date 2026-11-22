"""Release metadata must agree after preparation on Windows and Unix hosts."""
import hashlib
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class ReleasePreparation(unittest.TestCase):
    def test_crlf_lock_input_produces_attested_lf_and_updates_both_clients(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture = Path(directory)
            paths = [
                Path('Cargo.toml'), Path('Cargo.lock'), Path('THIRD_PARTY_NOTICES.md'),
                Path('tools/dev/prepare_release_identity.py'),
                Path('apps/macos/Info.plist'), Path('apps/desktop-egui/Info.plist'),
                Path('apps/windows/Cargo.toml'), Path('apps/desktop-egui/Cargo.toml'),
                *[p.relative_to(ROOT) for p in (ROOT / 'crates').glob('*/Cargo.toml')],
                *[Path('installers/windows') / name for name in (
                    'WhereIsItAI.iss', 'WhereIsItAI-EguiWin64.iss', 'WhereIsItAI-EguiWin64QA.iss')],
            ]
            for relative in paths:
                target = fixture / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(ROOT / relative, target)
            (fixture / 'VERSION').write_text('1.0.0-rc.5\n', encoding='utf-8')
            lock = fixture / 'Cargo.lock'
            lock.write_bytes(lock.read_bytes().replace(b'\r\n', b'\n').replace(b'\n', b'\r\n'))
            subprocess.run([sys.executable, str(fixture / paths[3])], check=True,
                           capture_output=True, timeout=30)
            self.assertNotIn(b'\r', lock.read_bytes())
            digest = hashlib.sha256(lock.read_bytes()).hexdigest()
            self.assertIn('SHA-256 Cargo.lock: `' + digest + '`',
                          (fixture / 'THIRD_PARTY_NOTICES.md').read_text(encoding='utf-8'))
            for app in ('macos', 'desktop-egui'):
                info = plistlib.loads((fixture / 'apps' / app / 'Info.plist').read_bytes())
                self.assertEqual(info['WhereIsItReleaseVersion'], '1.0.0-rc.5')
                self.assertEqual(info['CFBundleVersion'], '10005')
            for relative in paths[-3:]:
                self.assertIn('#define AppVersion "1.0.0-rc.5"',
                              (fixture / relative).read_text(encoding='utf-8'))


if __name__ == '__main__':
    unittest.main()
