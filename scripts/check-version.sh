#!/usr/bin/env bash
# Fails unless the tag, app/Cargo.toml, app/nsh.json and the workspace all name the same version.
set -euo pipefail
tag=${1:?usage: check-version.sh <tag>}
root="$(cd "$(dirname "$0")/.." && pwd)"
version=$("$root/scripts/app-version.sh")
nsh_json=$(jq -r .version "$root/app/nsh.json")
workspace=$(sed -nE 's/^version = "([^"]+)".*/\1/p' "$root/Cargo.toml" | head -n 1)
if [[ $tag != "v$version" ]]; then
  echo "❌ Tag \"$tag\" does not match app/Cargo.toml version \"$version\" (expected \"v$version\")." >&2
  exit 1
fi
if [[ $nsh_json != "$version" || $workspace != "$version" ]]; then
  echo "❌ Versions differ: app/Cargo.toml $version, app/nsh.json $nsh_json, Cargo.toml $workspace." >&2
  exit 1
fi
echo "✅ Tag $tag matches version $version."
