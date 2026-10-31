#!/usr/bin/env bash
# Builds target/release-dist/Toolbook.app with nsh, unsigned (nsh signs it ad hoc). Run it before any
# signing certificate is in the keychain: build scripts and proc macros never see an unlocked key.
# Usage: scripts/build-app.sh
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
dist="$root/target/release-dist"
version=$("$root/scripts/app-version.sh")
rm -rf "$dist"
mkdir -p "$dist"
(cd "$root/app" && nsh package . --target macos --out "$dist" \
  --id io.github.wenlingang.toolbook --app-version "$version" --build "${BUILD_NUMBER:-0}")
[[ -d "$dist/Toolbook.app" ]] || { echo "❌ nsh did not write $dist/Toolbook.app" >&2; exit 1; }
echo "$dist/Toolbook.app"
