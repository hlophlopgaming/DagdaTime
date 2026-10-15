"""Release launcher installation, isolated from the user's desktop."""
from pathlib import Path
import os
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class LinuxInstallTests(unittest.TestCase):
    def test_install_copies_executables_icon_and_matching_launcher(self):
        with tempfile.TemporaryDirectory(prefix='draugr install ') as directory:
            root = Path(directory)
            bundle = root / 'bundle'
            bundle.mkdir()
            for name in ('draugr-time', 'draugr-time-cli'):
                (bundle / name).write_text('#!/bin/sh\nexit 0\n')
            (bundle / 'draugr-time.png').write_bytes((ROOT / 'logo/blackDraugr128.png').read_bytes())
            env = dict(os.environ, XDG_DATA_HOME=str(root / 'data'))
            subprocess.run(['python3', str(ROOT / 'scripts/install-linux.py'), str(bundle)],
                           env=env, check=True, capture_output=True)
            data = root / 'data'
            executable = data / 'draugr-time/draugr-time'
            self.assertTrue(os.access(executable, os.X_OK))
            self.assertTrue((data / 'draugr-time/draugr-time-cli').is_file())
            icon = data / 'icons/hicolor/128x128/apps/draugr-time.png'
            self.assertEqual(icon.read_bytes(), (bundle / 'draugr-time.png').read_bytes())
            launcher = (data / 'applications/draugr-time.desktop').read_text()
            self.assertIn(f'Exec="{executable}"', launcher)
            self.assertIn('Icon=draugr-time\n', launcher)
            self.assertIn('StartupWMClass=draugr-time\n', launcher)

    def test_window_uses_the_launcher_application_id(self):
        source = (ROOT / 'app/src/desktop/mod.rs').read_text()
        self.assertIn('application_id: "draugr-time".into()', source)
