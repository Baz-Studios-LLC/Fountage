#!/bin/zsh
set -eu
cd -- "${0:A:h}"
./scripts/build-slint-lab.sh
open 'prototypes/slint/target/Fountage Slint Lab.app' --args "$@"
