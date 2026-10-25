#!/usr/bin/env python3
import os
from pathlib import Path
import shutil
import subprocess


def exec_path(path):
    value = str(path)
    for char in ('\\', '"', '`', '$'):
        value = value.replace(char, '\\' + char)
    return '"' + value.replace('\\', '\\\\').replace('%', '%%') + '"'


def desktop_value(path):
    return str(path).replace('\\', '\\\\').replace('\n', '\\n').replace('\r', '\\r')


root = Path(__file__).resolve().parent.parent
data_home = Path(os.environ.get('XDG_DATA_HOME') or Path.home() / '.local/share')
applications = data_home / 'applications'
applications.mkdir(parents=True, exist_ok=True)
entry = (root / 'packaging/app.suwa.Suwa.desktop').read_text()
entry = entry.replace('Exec=suwa\n', f'Exec=/usr/bin/bash {exec_path(root / "scripts/run.sh")}\n')
entry = entry.replace('Icon=app.suwa.Suwa\n', f'Icon={desktop_value(root / "packaging/app.suwa.Suwa.png")}\n')
destination = applications / 'app.suwa.Suwa.desktop'
destination.write_text(entry)
if shutil.which('update-desktop-database'):
    subprocess.run(['update-desktop-database', str(applications)], check=True)
print(f'Installed {destination}')
