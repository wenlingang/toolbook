# Toolbook

English | [简体中文](README.zh-CN.md)

A notebook of developer tools. One Markdown note holds your explanation and the tool blocks that do the work: JSON, CSV → SQL, JWT, HMAC, timestamps, regex, diff, HTTP, JS and more. Blocks pass data to each other with `$name`, and editing an upstream block recomputes everything downstream. The same note also runs in a terminal, or when Claude or Codex runs it.

Built on [Native Shell](https://nsh.dev). The v1 app is macOS-only, and its interface is in Chinese.

## A note

````markdown
---
toolbook: 1
---

# Why does login drop right away

Take the token from the request header and see when it expires.

```text name=header param
Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
```

```jwt-decode name=jwt input=$header
```

```timestamp name=expires input=$jwt.payload.exp
```
````

Each fenced block names a tool. `name=` gives its result a name, and `input=$header` reads another block's result. `param` marks the blocks a caller can fill in from outside, such as `--input` on the command line. The app shows each result under its block. The Markdown view shows the result next to the closing fence, and the preview shows the note as a page to share.

## Tools

| Tool | What it does |
|---|---|
| `text` | Plain text, usually a note's input |
| `http` | An HTTP request (runs only when you press ⌘↵) |
| `json` · `pick` · `filter` · `map` | Format JSON, take fields, keep rows, rename fields |
| `csv` · `to-json` · `to-sql` | CSV to a table, a table to JSON, to `INSERT`/`UPDATE` statements, or to your own SQL filled in per row (`{{ column }}`) |
| `base64` · `url` · `hash` · `hmac` · `jwt-decode` | Encode, decode, hash, sign |
| `timestamp` · `regex` · `diff` | Times across time zones, regex matches, text diffs |
| `js` | A JavaScript expression over earlier results |
| `flow` | Run another note as a step |
| `secret` | A secret: read from the environment or typed in the app, never written to the note |

## Install

**Mac app**

Download [Toolbook.dmg](https://github.com/wenlingang/toolbook/releases/latest/download/Toolbook.dmg) (Apple Silicon) and drag **Toolbook** into Applications. Do not run it from inside the dmg.

Toolbook is signed with its own certificate but not notarized by Apple, so macOS blocks the first launch. Either open System Settings → Privacy & Security and click **Open Anyway**, or, before opening the dmg, run `xattr -d com.apple.quarantine ~/Downloads/Toolbook.dmg`. Later versions install themselves: the app checks once a day, or choose **Toolbook › 检查更新…**.

**Command line**

The `toolbook` command is not in the dmg yet; build it with Rust (edition 2024):

```sh
cargo install --git https://github.com/wenlingang/toolbook toolbook-cli --locked
toolbook list --json
toolbook describe ~/Toolbook/数据转换/csv-to-sql.md --json
toolbook run ~/Toolbook/数据转换/csv-to-sql.md --input rows=@data.csv
cat data.csv | toolbook run csv-to-sql.md --input rows=@-
toolbook share csv-to-sql.md -o csv-to-sql.html   # the page to share
```

Exit codes: 0 means success. 1 means a block failed, and stderr names the block and the reason. 2 means a bad argument or file. A `secret` block reads `TOOLBOOK_SECRET_<NAME>`, and `--json` output masks it.

**Building the app from source**

The app needs the `nsh` command line built from the same commit it depends on:

```sh
cargo install --git https://github.com/longbridge/nsh --rev 844bf1ea368a50a32e25bd8ebbcb7ee413f0f0d0 nsh-cli --locked
git clone https://github.com/wenlingang/toolbook && cd toolbook/app
nsh run .                       # run in development
nsh package . --target macos    # build .nsh/out/Toolbook.app (unsigned)
```

Notes live in `~/Toolbook/`, or in `TOOLBOOK_HOME` when that is set. The first launch adds a few example notes. To add your own themes, put them in `~/Toolbook/.themes/*.json` (shadcn `cssVars` format).

## Use it with Claude or Codex

`skill/toolbook/SKILL.md` tells an agent how to find notes, read what inputs they take, and run them with `toolbook`. `toolbook export-skill <note>` turns a note into a Claude skill of its own (in `~/.claude/skills` by default).

## Development

| Directory | What it is |
|---|---|
| `crates/toolbook-core` | The engine: parses notes, orders blocks, runs the built-in tools, recomputes what changed. No nsh dependency |
| `crates/toolbook-cli` | The `toolbook` command: `run`, `describe`, `list`, `share`, `export-skill` |
| `app/` | The Mac app: a Rust editor and a TSX shell on nsh |
| `skill/` | The skill for agents |
| `examples/` | Example notes, also test fixtures |

```sh
cargo test                                            # core and CLI
cargo test --release -- --ignored                     # performance tests
cd app && cargo test                                  # the app's Rust tests
cd app && TOOLBOOK_HOME=$(mktemp -d)/lib nsh test .   # all app tests, TSX included
```

The app tests rename, move and trash notes. They refuse to run against `~/Toolbook`, so always point `TOOLBOOK_HOME` at an empty temporary folder.

## Releases and updates

Pushing a `v*` tag builds, signs and publishes a release (`.github/workflows/release.yml`). The app is signed with a long-lived self-signed certificate, "Toolbook Release", whose fingerprint is pinned in `scripts/release-cert.sha1`; it is not notarized by Apple. The app checks `releases/latest/download/latest.json` shortly after launch and once a day (or from **Toolbook › 检查更新…**), and installs an update only when it is signed by that same certificate and has the version the manifest names.

1. Once: `scripts/release-cert.sh`, commit `scripts/release-cert.sha1`, and set the `RELEASE_CERT_P12` and `RELEASE_CERT_PASSWORD` secrets as it says. Back up the certificate: losing it means users must install every later version by hand.
2. Each release: raise the version in `Cargo.toml`, `app/Cargo.toml` and `app/nsh.json`, then push an annotated tag; its message becomes the notes in the update dialog.

```sh
git tag -a v0.2.0 -m "Toolbook 0.2.0
- What changed"
git push origin v0.2.0
```

## License

[Apache-2.0](LICENSE)
