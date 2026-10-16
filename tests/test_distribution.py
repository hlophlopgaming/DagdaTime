import importlib.util
from pathlib import Path
import plistlib
import struct
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]

class DistributionTests(unittest.TestCase):
    def test_native_icons_wrap_the_existing_png(self):
        png = (ROOT / 'logo/blackDraugr128.png').read_bytes()
        ico = (ROOT / 'logo/draugr-time.ico').read_bytes()
        self.assertEqual(struct.unpack('<HHH', ico[:6]), (0, 1, 1))
        self.assertEqual(ico[22:], png)
        icns = (ROOT / 'logo/draugr-time.icns').read_bytes()
        self.assertEqual(icns[:4], b'icns')
        self.assertEqual(struct.unpack('>I', icns[4:8])[0], len(icns))
        self.assertEqual(icns[16:], png)

    def test_mac_archive_contains_launchable_bundle_metadata_and_executable(self):
        spec = importlib.util.spec_from_file_location('bundle', ROOT / 'scripts/package_macos.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            source = folder / 'draugr-time'
            source.write_bytes(b'fake executable')
            destination = folder / 'Draugr Time.app'
            module.create_bundle(source, destination, '1.0.0')
            contents = destination / 'Contents'
            info = plistlib.loads((contents / 'Info.plist').read_bytes())
            self.assertEqual(info['CFBundleIdentifier'], 'org.draugrtime.app')
            self.assertEqual(info['CFBundleShortVersionString'], '1.0.0')
            self.assertEqual((contents / 'MacOS' / info['CFBundleExecutable']).read_bytes(), source.read_bytes())
            self.assertTrue((contents / 'Resources' / info['CFBundleIconFile']).is_file())
