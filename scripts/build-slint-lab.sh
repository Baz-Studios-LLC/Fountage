#!/bin/sh
# Prepare the separate Mac prototype; never replace a running app or change file associations.
set -eu
cd "$(dirname "$0")/.."
fountage_root=$(pwd -P)
if [ "$(uname -s)" != Darwin ]; then
    echo 'Use cargo run --manifest-path prototypes/slint/Cargo.toml on this platform.' >&2
    exit 1
fi
python3 scripts/package-slint-lab.py --check
cargo build --manifest-path prototypes/slint/Cargo.toml
icon_output=$(mktemp -d "${TMPDIR:-/tmp}/fountage-slint-icons.XXXXXX")
trap 'rm -rf "$icon_output"' EXIT HUP INT TERM
xcrun actool "$fountage_root/macOS/Resources/Fountage.icon" \
    --compile "$icon_output" --app-icon Fountage \
    --output-partial-info-plist "$icon_output/icon-info.plist" \
    --platform macosx --minimum-deployment-target 14.0 --target-device mac \
    --output-format human-readable-text
python3 scripts/package-slint-lab.py "$icon_output"
