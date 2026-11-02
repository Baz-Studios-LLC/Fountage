#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
release_label=$(cat VERSION)
release_digits=${release_label#v}
case "$release_label" in v*) ;; *) printf 'VERSION must look like v001\n' >&2; exit 1 ;; esac
case "$release_digits" in ''|*[!0-9]*) printf 'VERSION must contain digits after v\n' >&2; exit 1 ;; esac
if [ "${#release_digits}" -lt 3 ]; then
    printf 'VERSION needs at least three digits\n' >&2
    exit 1
fi
release_number=$(printf '%s' "$release_digits" | sed 's/^0*//')
if [ -z "$release_number" ]; then
    printf 'VERSION must be greater than v000\n' >&2
    exit 1
fi
cargo build --release --lib
swift build --package-path macOS
icon_output=$(mktemp -d "${TMPDIR:-/tmp}/fountage-icons.XXXXXX")
trap 'rm -rf "$icon_output"' EXIT HUP INT TERM
# Compile the layered icon for current macOS and generate the legacy .icns fallback.
xcrun actool macOS/Resources/Fountage.icon \
    --compile "$icon_output" --app-icon Fountage \
    --output-partial-info-plist "$icon_output/icon-info.plist" \
    --platform macosx --minimum-deployment-target 14.0 --target-device mac \
    --output-format human-readable-text
bundle="target/Fountage.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
for resource in Assets.car Fountage.icns; do
    cp "$icon_output/$resource" "$bundle/Contents/Resources/$resource.next"
    mv "$bundle/Contents/Resources/$resource.next" "$bundle/Contents/Resources/$resource"
done
# Replace the executable atomically so an already-running preview keeps its current binary.
cp macOS/.build/debug/Fountage "$bundle/Contents/MacOS/Fountage.next"
mv "$bundle/Contents/MacOS/Fountage.next" "$bundle/Contents/MacOS/Fountage"
cat > "$bundle/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>Fountage</string>
<key>CFBundleIdentifier</key><string>com.bazstudios.fountage</string>
<key>CFBundleName</key><string>Fountage</string>
<key>CFBundleDisplayName</key><string>Fountage</string>
<key>CFBundleIconName</key><string>Fountage</string>
<key>CFBundleIconFile</key><string>Fountage.icns</string>
<key>CFBundleVersion</key><string>${release_number}</string>
<key>CFBundleShortVersionString</key><string>0.0.${release_number}</string>
<key>FountageReleaseVersion</key><string>${release_label}</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>LSMinimumSystemVersion</key><string>14.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSHumanReadableCopyright</key><string>Fountage by Baz Studios</string>
<key>CFBundleDocumentTypes</key><array>
<dict><key>CFBundleTypeName</key><string>Fountage Project</string>
<key>CFBundleTypeIconFile</key><string>Fountage.icns</string>
<key>CFBundleTypeRole</key><string>Editor</string><key>LSHandlerRank</key><string>Owner</string>
<key>LSTypeIsPackage</key><true/>
<key>LSItemContentTypes</key><array><string>com.bazstudios.fountage.project</string></array></dict>
<dict><key>CFBundleTypeName</key><string>Fountain Screenplay</string>
<key>CFBundleTypeRole</key><string>Editor</string><key>LSHandlerRank</key><string>Alternate</string>
<key>LSItemContentTypes</key><array><string>com.fountain.screenplay</string></array></dict>
<dict><key>CFBundleTypeName</key><string>Plain Text</string>
<key>CFBundleTypeRole</key><string>Editor</string><key>LSHandlerRank</key><string>Alternate</string>
<key>LSItemContentTypes</key><array><string>public.plain-text</string></array></dict>
</array>
<key>UTExportedTypeDeclarations</key><array><dict>
<key>UTTypeIdentifier</key><string>com.bazstudios.fountage.project</string>
<key>UTTypeDescription</key><string>Fountage Project</string>
<key>UTTypeIconFile</key><string>Fountage.icns</string>
<key>UTTypeConformsTo</key><array><string>org.textbundle.package</string><string>com.apple.package</string></array>
<key>UTTypeTagSpecification</key><dict><key>public.filename-extension</key><array><string>fountage</string></array></dict>
</dict></array>
<key>UTImportedTypeDeclarations</key><array><dict>
<key>UTTypeIdentifier</key><string>com.fountain.screenplay</string>
<key>UTTypeDescription</key><string>Fountain Screenplay</string>
<key>UTTypeConformsTo</key><array><string>public.plain-text</string></array>
<key>UTTypeTagSpecification</key><dict><key>public.filename-extension</key><array><string>fountain</string></array></dict>
</dict></array>
</dict></plist>
PLIST
# Refresh this bundle's icons and document declaration without changing default handlers.
touch "$bundle"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$PWD/$bundle"
printf 'Built %s\n' "$PWD/$bundle"
