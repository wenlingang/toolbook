# Toolbook

[English](README.md) | 简体中文

开发者的工具笔记本。一篇 Markdown 笔记里写说明文字，也放干活的工具块：JSON、CSV → SQL、JWT、HMAC、时间戳、正则、Diff、HTTP、JS 等等。块之间用 `$名字` 传数据，改了上游，下游自动重算。同一篇笔记也能在终端里运行，或者交给 Claude / Codex 运行。

基于 [Native Shell](https://nsh.dev)。v1 的应用只支持 macOS，界面是中文。

## 一篇笔记

````markdown
---
toolbook: 1
---

# 排查登录 401

用户反馈登录后马上掉线：从请求头里取出 token，看它什么时候过期。

```text name=header param
Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
```

```jwt-decode name=jwt input=$header
```

```timestamp name=expires input=$jwt.payload.exp
```
````

每个代码块的第一个词是工具名。`name=` 给它的结果起个名字，`input=$header` 读取别的块的结果。`param` 标出调用方可以从外面填的块，比如命令行的 `--input`。应用里，结果显示在块的下面；Markdown 视图里，结果显示在块结束行的后面；预览里，整篇笔记显示成一个可以分享的页面。

## 工具

| 工具 | 做什么 |
|---|---|
| `text` | 纯文本，通常是笔记的输入 |
| `http` | 发 HTTP 请求（按 ⌘↵ 时才发） |
| `json` · `pick` · `filter` · `map` | 格式化 JSON、取字段、筛选行、改字段名 |
| `csv` · `to-json` · `to-sql` | CSV 转表格，表格转 JSON、`INSERT`/`UPDATE` 语句，或按行填进自己写的 SQL 模板（`{{ 列名 }}`） |
| `base64` · `url` · `hash` · `hmac` · `jwt-decode` | 编码、解码、哈希、签名 |
| `timestamp` · `regex` · `diff` | 跨时区的时间、正则匹配、文本对比 |
| `js` | 对前面块的结果算一个 JavaScript 表达式 |
| `flow` | 把另一篇笔记当作一步来运行 |
| `secret` | 密钥：从环境变量读取或在应用里输入，不会写进笔记 |

## 安装

**Mac 应用**

下载 [Toolbook.dmg](https://github.com/wenlingang/toolbook/releases/latest/download/Toolbook.dmg)（Apple 芯片），把 **Toolbook** 拖进「应用程序」。不要直接在 dmg 里运行。

Toolbook 用自己的证书签了名，但没有经过 Apple 公证，所以第一次打开会被 macOS 拦下。两种办法任选：到「系统设置 → 隐私与安全性」里点「仍要打开」；或者在打开 dmg 之前运行 `xattr -d com.apple.quarantine ~/Downloads/Toolbook.dmg`。之后的版本会自动更新：应用每天检查一次，也可以点 **Toolbook › 检查更新…**。

**命令行**

`toolbook` 命令暂时不在 dmg 里，需要用 Rust（edition 2024）编译：

```sh
cargo install --git https://github.com/wenlingang/toolbook toolbook-cli --locked
toolbook list --json
toolbook describe ~/Toolbook/数据转换/csv-to-sql.md --json
toolbook run ~/Toolbook/数据转换/csv-to-sql.md --input rows=@data.csv
cat data.csv | toolbook run csv-to-sql.md --input rows=@-
toolbook share csv-to-sql.md -o csv-to-sql.html   # 生成分享页
```

退出码：0 表示成功；1 表示某个块出错，stderr 里写明是第几块、什么原因；2 表示参数或文件有错。`secret` 块从 `TOOLBOOK_SECRET_<NAME>` 读取，`--json` 输出里会打码。

**从源码构建应用**

应用需要用同一个 commit 编出来的 `nsh` 命令行：

```sh
cargo install --git https://github.com/longbridge/nsh --rev 844bf1ea368a50a32e25bd8ebbcb7ee413f0f0d0 nsh-cli --locked
git clone https://github.com/wenlingang/toolbook && cd toolbook/app
nsh run .                       # 开发运行
nsh package . --target macos    # 生成 .nsh/out/Toolbook.app（未签名）
```

笔记放在 `~/Toolbook/`；设置了 `TOOLBOOK_HOME` 时放在那个目录。第一次启动会放入几篇示例笔记。自定义主题放在 `~/Toolbook/.themes/*.json`（shadcn 的 `cssVars` 格式）。

## 配合 Claude / Codex 使用

`skill/toolbook/SKILL.md` 告诉 agent 怎么找到笔记、看笔记需要哪些输入，以及怎么用 `toolbook` 运行它。`toolbook export-skill <笔记>` 会把一篇笔记导出成单独的 Claude skill（默认放在 `~/.claude/skills`）。

## 开发

| 目录 | 是什么 |
|---|---|
| `crates/toolbook-core` | 引擎：解析笔记、排好块的顺序、运行内置工具、只重算改动的部分。不依赖 nsh |
| `crates/toolbook-cli` | `toolbook` 命令：`run`、`describe`、`list`、`share`、`export-skill` |
| `app/` | Mac 应用：基于 nsh，Rust 写编辑器，TSX 写外壳 |
| `skill/` | 给 agent 的 skill |
| `examples/` | 示例笔记，也是测试夹具 |

```sh
cargo test                                            # core 和 CLI
cargo test --release -- --ignored                     # 性能测试
cd app && cargo test                                  # 应用的 Rust 测试
cd app && TOOLBOOK_HOME=$(mktemp -d)/lib nsh test .   # 应用全部测试（含 TSX）
```

应用的测试会改名、移动、删除笔记。如果用的是 `~/Toolbook`，测试会拒绝运行，所以一定要把 `TOOLBOOK_HOME` 指到一个空的临时目录。

## 发布与更新

推送 `v*` 标签会自动构建、签名并发布（`.github/workflows/release.yml`）。应用用一张长期有效的自签名证书「Toolbook Release」签名，证书指纹固定在 `scripts/release-cert.sha1`；没有经过 Apple 公证。应用启动后不久检查一次，之后每天一次（也可以点 **Toolbook › 检查更新…**），读取 `releases/latest/download/latest.json`；只有新版由同一张证书签名、而且版本号和清单一致时才会安装。

1. 只做一次：运行 `scripts/release-cert.sh`，提交 `scripts/release-cert.sha1`，再按它的提示设置 `RELEASE_CERT_P12` 和 `RELEASE_CERT_PASSWORD` 两个 secret。证书一定要备份：丢了以后，用户只能手动安装之后的每个版本。
2. 每次发布：把 `Cargo.toml`、`app/Cargo.toml`、`app/nsh.json` 里的版本号改好，再推送一个带注释的标签；注释会显示在更新对话框里。

```sh
git tag -a v0.2.0 -m "Toolbook 0.2.0
- 改了什么"
git push origin v0.2.0
```

## 许可证

[Apache-2.0](LICENSE)
