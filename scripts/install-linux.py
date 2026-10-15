#!/usr/bin/env python3
"""Install the extracted Linux release for the current user, without root."""
import os
from pathlib import Path
import shutil
import sys


def install(bundle):
    data = Path(os.environ.get('XDG_DATA_HOME') or Path.home() / '.local/share').resolve()
    destination = data / 'draugr-time'
    destination.mkdir(parents=True, exist_ok=True)
    for name in ('draugr-time', 'draugr-time-cli'):
        target = destination / name
        # Replace by rename so upgrading a running executable works on Linux.
        temporary = target.with_suffix('.new')
        shutil.copyfile(bundle / name, temporary)
        temporary.chmod(0o755)
        temporary.replace(target)
    icons = data / 'icons/hicolor/128x128/apps'
    icons.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(bundle / 'draugr-time.png', icons / 'draugr-time.png')
    applications = data / 'applications'
    applications.mkdir(parents=True, exist_ok=True)
    executable = str(destination / 'draugr-time')
    # Desktop Entry Exec quoting is separate from shell quoting; % denotes field codes.
    for character in ('\\', '"', '`', '$'):
        executable = executable.replace(character, '\\' + character)
    executable = executable.replace('%', '%%').replace('\\', '\\\\')
    (applications / 'draugr-time.desktop').write_text(
        '[Desktop Entry]\nType=Application\nName=Draugr Time\n'
        f'Exec="{executable}"\nIcon=draugr-time\n'
        'StartupWMClass=draugr-time\nTerminal=false\nCategories=Office;\n',
        encoding='utf-8',
    )
    print(f'Installed Draugr Time: {destination}')
    print('Open Draugr Time from your application menu.')


if __name__ == '__main__':
    install(Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(__file__).resolve().parent)
