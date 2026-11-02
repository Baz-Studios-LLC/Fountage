"""Package Slint Lab with independent identity and current approved icon resources."""
import plistlib
import re
import shutil
import subprocess
import sys
from pathlib import Path

root = Path(__file__).resolve().parent.parent
bundle = root / 'prototypes/slint/target/Fountage Slint Lab.app'
contents = bundle / 'Contents'
executable = contents / 'MacOS/fountage-slint-lab'
identity = 'com.bazstudios.fountage.slintlab'


def check():
    if bundle.exists():
        info = plistlib.loads((contents / 'Info.plist').read_bytes())
        if info.get('CFBundleIdentifier') != identity:
            raise SystemExit('Refusing to replace a bundle with a different identity.')
    commands = subprocess.run(['ps', '-axo', 'command='], check=True, capture_output=True, text=True).stdout.splitlines()
    if any(line.strip() == str(executable) or line.strip().startswith(str(executable) + ' ') for line in commands):
        raise SystemExit('Slint Lab is running. Save any scratch work and quit it normally before rebuilding. The production Fountage app can stay open.')


if __name__ == '__main__':
    check()
    if sys.argv[1:] == ['--check']:
        raise SystemExit(0)
    if len(sys.argv) != 2:
        raise SystemExit('Usage: package-slint-lab.py ICON_OUTPUT_DIR | --check')
    label = (root / 'VERSION').read_text().strip()
    if not re.fullmatch(r'v[0-9]{3,}', label) or int(label[1:]) < 1:
        raise SystemExit('Invalid release label in VERSION')
    icons = Path(sys.argv[1])
    sources = [(root / 'prototypes/slint/target/debug/fountage-slint-lab', executable)]
    sources += [(icons / name, contents / 'Resources' / name) for name in ('Fountage.icns', 'Assets.car')]
    for source, _ in sources:
        if not source.is_file():
            raise SystemExit(f'Missing build resource: {source}')
    check()
    for source, destination in sources:
        destination.parent.mkdir(parents=True, exist_ok=True)
        temporary = destination.with_name(destination.name + '.next')
        shutil.copy2(source, temporary)
        temporary.replace(destination)
    info = dict(CFBundleIdentifier=identity, CFBundleName='Fountage Slint Lab',
                CFBundleDisplayName='Fountage Slint Lab', CFBundleExecutable='fountage-slint-lab',
                CFBundlePackageType='APPL', CFBundleIconFile='Fountage.icns', CFBundleIconName='Fountage',
                CFBundleShortVersionString=f'0.0.{int(label[1:])}', CFBundleVersion=str(int(label[1:])),
                FountageReleaseVersion=label, NSHighResolutionCapable=True, LSMinimumSystemVersion='14.0',
                NSHumanReadableCopyright='Fountage by Baz Studios')
    (contents / 'Info.plist').write_bytes(plistlib.dumps(info))
    bundle.touch()
    print(bundle)
