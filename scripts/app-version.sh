#!/usr/bin/env bash
# The app's version: app/Cargo.toml's (the app reads it as CARGO_PKG_VERSION to compare with releases).
set -euo pipefail
cd "$(dirname "$0")/../app"
cargo pkgid | sed 's/.*[#@]//'
