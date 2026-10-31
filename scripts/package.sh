#!/usr/bin/env bash
# Signs target/release-dist/Toolbook.app (from build-app.sh) with the release certificate, puts it in
# Toolbook-<version>.dmg with its SHA-256, and checks the result.
# Usage: scripts/package.sh [--identity <name>]   (default: "Toolbook Release", checked against its pin)
set -euo pipefail

identity="Toolbook Release"
while [[ $# -gt 0 ]]; do
  case $1 in
    --identity) identity=${2:?--identity needs a value}; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
  esac
done

root="$(cd "$(dirname "$0")/.." && pwd)"
dist="$root/target/release-dist"
app="$dist/Toolbook.app"
pin_args=()

if [[ $identity == "Toolbook Release" ]]; then
  pin_file="$root/scripts/release-cert.sha1"
  if [[ ! -s $pin_file ]]; then
    echo "❌ scripts/release-cert.sha1 is missing: run scripts/release-cert.sh once and commit the pin file it writes." >&2
    exit 1
  fi
  pin_args=("$pin_file")
fi
[[ -d $app ]] || { echo "❌ $app is missing: run scripts/build-app.sh first." >&2; exit 1; }
if ! security find-identity -p codesigning | grep -q "\"$identity\""; then
  echo "❌ Signing identity \"$identity\" not found in the keychain search list." >&2
  exit 1
fi

version="$("$root/scripts/app-version.sh")"
dmg="$dist/Toolbook-$version.dmg"

codesign --force --timestamp=none --sign "$identity" "$app"

staging="$dist/dmg"
rm -rf "$staging" "$dmg" "$dmg.sha256"
mkdir -p "$staging"
cp -R "$app" "$staging/"
ln -s /Applications "$staging/Applications"
hdiutil create -quiet -volname Toolbook -srcfolder "$staging" -ov -format UDZO "$dmg"
rm -rf "$staging"
(cd "$dist" && shasum -a 256 "$(basename "$dmg")" > "$(basename "$dmg").sha256")

"$root/scripts/verify-package.sh" "$app" "$dmg" "$identity" "$version" ${pin_args[@]+"${pin_args[@]}"}
echo "$dmg"
