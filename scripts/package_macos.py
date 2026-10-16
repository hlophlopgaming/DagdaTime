"""Build the .app directory for the portable macOS release archive."""
from pathlib import Path
import plistlib
import shutil
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def create_bundle(executable: Path, destination: Path, version: str):
    contents = destination / 'Contents'
    (contents / 'MacOS').mkdir(parents=True, exist_ok=True)
    (contents / 'Resources').mkdir(exist_ok=True)
    shutil.copy2(executable, contents / 'MacOS/draugr-time')
    shutil.copy2(ROOT / 'logo/draugr-time.icns', contents / 'Resources/draugr-time.icns')
    info = {
        'CFBundleName': 'Draugr Time',
        'CFBundleDisplayName': 'Draugr Time',
        'CFBundleIdentifier': 'org.draugrtime.app',
        'CFBundleExecutable': 'draugr-time',
        'CFBundleIconFile': 'draugr-time.icns',
        'CFBundlePackageType': 'APPL',
        'CFBundleShortVersionString': version,
        'CFBundleVersion': version,
        'NSHighResolutionCapable': True,
    }
    (contents / 'Info.plist').write_bytes(plistlib.dumps(info))


if __name__ == '__main__':
    release = ROOT / 'target/release'
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['package']['version']
    create_bundle(release / 'draugr-time', release / 'Draugr Time.app', version)
