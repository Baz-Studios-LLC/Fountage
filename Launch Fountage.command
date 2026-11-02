#!/bin/zsh

# Finder may start Terminal outside the project and without Rust on PATH.
cd -- "${0:A:h}" || exit 1
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

printf 'Building Fountage for testing…\n\n'
if ! /bin/sh scripts/run-macos.sh; then
    printf '\nBuild failed. See the error above. Press Return to close.\n'
    read -r
    exit 1
fi

if ! /usr/bin/open "$PWD/target/Fountage.app"; then
    printf '\nCould not open Fountage. Press Return to close.\n'
    read -r
    exit 1
fi

printf '\nFountage opened. You can close this Terminal window.\n'
printf 'If Fountage was already running, quit it normally and double-click this launcher again to use the new build.\n'
