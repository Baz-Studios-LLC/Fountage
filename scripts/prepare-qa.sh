#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
# Build first with run-macos.sh. This prepares but does not launch a test app.
# Keep one stable identity so QA preferences and saved test windows remain separate.
python3 - "$@" <<'PY'
import plistlib
import shutil
import subprocess
import sys
from pathlib import Path

if len(sys.argv) != 2:
    raise SystemExit('Usage: scripts/prepare-qa.sh "/absolute/path/Fountage Goal QA.app"')
destination = Path(sys.argv[1])
source = Path('target/Fountage.app').resolve()
if not destination.is_absolute() or destination.suffix != '.app' or destination.resolve() == source:
    raise SystemExit('Choose an absolute QA .app path separate from the production bundle.')
if destination.exists():
    info = plistlib.loads((destination / 'Contents/Info.plist').read_bytes())
    if info.get('CFBundleIdentifier') != 'com.bazstudios.fountage.goalqa':
        raise SystemExit('Refusing to replace an app that is not the Fountage QA copy.')
processes = subprocess.run(['ps', '-axo', 'command='], capture_output=True, text=True, check=True).stdout.splitlines()
if any(line.strip().startswith(str(destination / 'Contents/MacOS/Fountage')) for line in processes):
    raise SystemExit('Quit the QA app normally before preparing its next build.')
for name in ['Contents/MacOS/Fountage', 'Contents/Resources/Fountage.icns', 'Contents/Resources/Assets.car']:
    original = source / name
    target = destination / name
    target.parent.mkdir(parents=True, exist_ok=True)
    temporary = target.with_name(target.name + '.next')
    shutil.copy2(original, temporary)
    temporary.replace(target)
info = plistlib.loads((source / 'Contents/Info.plist').read_bytes())
info.update(CFBundleIdentifier='com.bazstudios.fountage.goalqa', CFBundleName='Fountage Goal QA', CFBundleDisplayName='Fountage Goal QA', FountageQAApplication=True)
for declaration in info.get('CFBundleDocumentTypes', []):
    declaration['LSHandlerRank'] = 'None'
# Production owns these types; QA only reads them.
info.pop('UTExportedTypeDeclarations', None)
(destination / 'Contents/Info.plist').write_bytes(plistlib.dumps(info))
destination.touch()
subprocess.run(['/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister', '-f', str(destination)], check=True)
print(f'Prepared {destination}')
PY
