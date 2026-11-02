"""Package a built Slint preview without modifying development app bundles."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parent.parent


def run(*args):
    # Cargo metadata is UTF-8 regardless of the Windows console code page.
    return subprocess.check_output(args, cwd=ROOT, encoding="utf-8")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--platform", choices=["macos", "windows"], required=True)
    parser.add_argument("--arch", choices=["arm64", "x64"], required=True)
    parser.add_argument("--binary", type=Path, required=True)
    options = parser.parse_args()
    version = (ROOT / "VERSION").read_text().strip()
    if not re.fullmatch(r"v\d{3,}", version):
        raise SystemExit("Invalid VERSION")
    binary = options.binary.resolve(strict=True)
    dist = ROOT / "dist"
    dist.mkdir(exist_ok=True)
    name = f"Fountage-{version}-{options.platform}-{options.arch}"
    destination = dist / f"{name}.zip"
    if destination.exists():
        raise SystemExit(f"Refusing to overwrite {destination}")
    with tempfile.TemporaryDirectory(prefix="fountage-release-") as temp:
        package = Path(temp) / name
        package.mkdir()
        shutil.copy2(ROOT / "docs/PREVIEW_RELEASE.md", package / "READ ME.md")
        shutil.copy2(ROOT / "THIRD_PARTY_NOTICES.md", package)
        shutil.copy2(ROOT / "VERSION", package)
        licenses = package / "THIRD-PARTY-LICENSES"
        licenses.mkdir()
        metadata = json.loads(run("cargo", "metadata", "--locked", "--format-version", "1", "--manifest-path", "prototypes/slint/Cargo.toml"))
        inventory = []
        for dependency in metadata["packages"]:
            if dependency["name"] in ("fountage", "fountage-slint-lab"):
                continue
            source = Path(dependency["manifest_path"]).parent
            texts = set()
            for pattern in ("LICENSE*", "LICENCE*", "COPYING*", "NOTICE*", "license*", "copyright*"):
                for item in source.glob(pattern):
                    if item.is_file():
                        texts.add(item)
                    elif item.is_dir():
                        texts.update(p for p in item.rglob("*") if p.is_file())
            if dependency.get("license_file"):
                texts.add(source / dependency["license_file"])
            target = licenses / f"{dependency['name']}-{dependency['version']}"
            for text in texts:
                if text.is_file():
                    relative = text.relative_to(source)
                    (target / relative).parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(text, target / relative)
            inventory.append({"name": dependency["name"], "version": dependency["version"], "license": dependency["license"], "repository": dependency["repository"], "texts": sorted(str(p.relative_to(source)) for p in texts if p.is_file())})
        (licenses / "inventory.json").write_text(json.dumps(inventory, indent=2) + "\n")
        if options.platform == "macos":
            app = package / "Fountage.app"
            contents = app / "Contents"
            (contents / "MacOS").mkdir(parents=True)
            (contents / "Resources").mkdir()
            executable = contents / "MacOS/Fountage"
            shutil.copy2(binary, executable)
            executable.chmod(0o755)
            for resource in ("Fountage.icns", "Assets.car"):
                shutil.copy2(ROOT / "packaging/icons" / resource, contents / "Resources" / resource)
            info = dict(CFBundleIdentifier="com.bazstudios.fountage.slintlab", CFBundleName="Fountage", CFBundleDisplayName="Fountage", CFBundleExecutable="Fountage", CFBundlePackageType="APPL", CFBundleIconFile="Fountage.icns", CFBundleIconName="Fountage", CFBundleShortVersionString=f"0.0.{int(version[1:])}", CFBundleVersion=str(int(version[1:])), FountageReleaseVersion=version, LSMinimumSystemVersion="14.0", NSHighResolutionCapable=True, NSHumanReadableCopyright="Fountage by Baz Studios")
            # Recognize .fountage directories as packages on a clean Mac, even
            # without the Swift reference installed. Do not claim Finder Open
            # handling: this preview currently opens projects through its menu.
            info["UTExportedTypeDeclarations"] = [dict(
                UTTypeIdentifier="com.bazstudios.fountage.project",
                UTTypeDescription="Fountage Project",
                UTTypeIconFile="Fountage.icns",
                UTTypeConformsTo=["com.apple.package"],
                UTTypeTagSpecification={"public.filename-extension": ["fountage"]},
            )]
            info["CFBundleDocumentTypes"] = [dict(
                CFBundleTypeName="Fountage Project", CFBundleTypeRole="None",
                CFBundleTypeIconFile="Fountage.icns", LSTypeIsPackage=True,
                LSItemContentTypes=["com.bazstudios.fountage.project"],
            )]
            (contents / "Info.plist").write_bytes(plistlib.dumps(info))
            subprocess.run(["codesign", "--force", "--deep", "--sign", "-", str(app)], check=True)
            subprocess.run(["codesign", "--verify", "--deep", "--strict", str(app)], check=True)
            subprocess.run(["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", str(package), str(destination)], check=True)
        else:
            shutil.copy2(binary, package / "Fountage.exe")
            with zipfile.ZipFile(destination, "x", zipfile.ZIP_DEFLATED) as archive:
                for path in sorted(package.rglob("*")):
                    if path.is_file():
                        archive.write(path, path.relative_to(package.parent))
        checksum = hashlib.sha256(destination.read_bytes()).hexdigest()
        (dist / f"{name}.sha256").write_text(f"{checksum}  {destination.name}\n")
        print(destination)


if __name__ == "__main__":
    main()
