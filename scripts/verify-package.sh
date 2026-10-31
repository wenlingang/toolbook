#!/usr/bin/env bash
# Checks a packaged release: signed by the right certificate (pinned), right bundle id and version,
# and a dmg holding the app and an Applications link.
# Usage: scripts/verify-package.sh <app> <dmg> <identity> <version> [<pin-file>]
set -euo pipefail
app=${1:?usage: verify-package.sh <app> <dmg> <identity> <version> [<pin-file>]}
dmg=${2:?}
identity=${3:?}
version=${4:?}
pin_file=${5:-}
bundle_id="io.github.wenlingang.toolbook"
fail() { echo "❌ $*" >&2; exit 1; }

archs=$(lipo -archs "$app/Contents/MacOS/Toolbook" 2>&1) \
  || fail "Cannot read architectures of $app/Contents/MacOS/Toolbook: $archs"
[[ " $archs " == *" arm64 "* ]] || fail "Binary must contain arm64, found: $archs"

verify_err=$(codesign --verify --strict --deep "$app" 2>&1) \
  || fail "codesign --verify failed for $app: $verify_err"
info=$(codesign -dvv "$app" 2>&1)
grep -qxF "Authority=$identity" <<<"$info" || fail "App is not signed by \"$identity\""
grep -qxF "Identifier=$bundle_id" <<<"$info" || fail "Signing identifier is not $bundle_id"
requirement=$(codesign -d -r- "$app" 2>&1)
grep -Eq 'certificate (leaf|root) = H"' <<<"$requirement" \
  || fail "Designated requirement does not pin a certificate leaf: $requirement"
if [[ -n $pin_file ]]; then
  pin=$(tr -d '[:space:]' < "$pin_file" 2>/dev/null || true)
  [[ -n $pin ]] || fail "Certificate pin file $pin_file missing or empty"
  signed=$(sed -nE 's/.*certificate (leaf|root) = H"([0-9A-Fa-f]+)".*/\2/p' <<<"$requirement" | head -n 1)
  [[ $(tr a-f A-F <<<"$signed") == "$(tr a-f A-F <<<"$pin")" ]] \
    || fail "Certificate pin mismatch: signed with H\"$signed\", expected $pin (scripts/release-cert.sha1)"
fi

plist_version=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app/Contents/Info.plist" 2>&1) \
  || fail "Cannot read CFBundleShortVersionString from $app/Contents/Info.plist: $plist_version"
[[ $plist_version == "$version" ]] || fail "Info.plist version is $plist_version, expected $version"

mnt=$(mktemp -d)
trap 'hdiutil detach -quiet "$mnt" 2>/dev/null || true; rmdir "$mnt" 2>/dev/null || true' EXIT
hdiutil attach -quiet -nobrowse -readonly -mountpoint "$mnt" "$dmg" || fail "Cannot mount dmg $dmg"
[[ -d "$mnt/Toolbook.app" && -L "$mnt/Applications" ]] \
  || fail "dmg must contain Toolbook.app and an Applications link"

echo "✅ Package verified: $identity, $archs, version $version${pin_file:+, certificate pin $pin}"
