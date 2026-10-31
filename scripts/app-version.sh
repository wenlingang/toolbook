#!/usr/bin/env bash
# The app's version: app/Cargo.toml's (the app reads it as CARGO_PKG_VERSION to compare with releases).
# Read from the file, so the signing job needs no cargo and fetches nothing.
set -euo pipefail
sed -nE 's/^version = "([^"]+)".*/\1/p' "$(dirname "$0")/../app/Cargo.toml" | head -n 1
