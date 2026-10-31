#!/usr/bin/env bash
# How to install, for the GitHub release page.
# Usage: scripts/release-notes.sh <version> <sha256>
set -euo pipefail
version=${1:?usage: release-notes.sh <version> <sha256>}
sha=${2:?}
cat <<EOT

1. Download \`Toolbook-$version.dmg\` and drag **Toolbook** into Applications. Do not run it from inside the dmg.
2. Toolbook is not notarized by Apple, so macOS blocks the first launch. Either:
   - open System Settings › Privacy & Security and click **Open Anyway** at the bottom, or
   - before opening the dmg, run \`xattr -d com.apple.quarantine ~/Downloads/Toolbook-$version.dmg\` in Terminal.
3. Later versions install themselves: Toolbook checks once a day, or choose **Toolbook › 检查更新…**.

简体中文安装说明：[README.zh-CN.md](https://github.com/wenlingang/toolbook/blob/main/README.zh-CN.md#安装)

Apple Silicon only. The app's interface is in Chinese.

SHA-256 (\`Toolbook-$version.dmg\`): \`$sha\`
EOT
