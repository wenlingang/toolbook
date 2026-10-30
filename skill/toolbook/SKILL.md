---
name: toolbook
description: 用 Toolbook 笔记处理开发数据——JSON 格式化/取字段/筛选、CSV 转 JSON 或 SQL、Base64/URL/JWT 解码、Hash/HMAC 签名核对、时间戳转换、正则、文本或 JSON Diff、HTTP 请求，或者执行用户在 ~/Toolbook 里保存的工具流。用户提到 toolbook、工具流、笔记库，或要做上面这类一次性数据转换时使用。
---

# Toolbook

`toolbook` 命令执行 Toolbook 笔记：一篇 Markdown，里面的代码块是工具（`json`、`csv`、`to-sql`、`jwt-decode`、`hmac`、`timestamp`、`js` 等），块之间用 `$名字` 传数据。它**从不交互提问**，适合直接调用。

## 先看用户已有的工具流

```sh
toolbook list --json                      # 笔记库（~/Toolbook，或 $TOOLBOOK_HOME）里的所有笔记
toolbook describe <路径> --json            # 它要哪些输入（inputs）、能输出哪些块（outputs）
toolbook run <路径> --input 名字=值 --json  # 执行
```

- 值以 `@` 开头表示读文件：`--input rows=@data.csv`；`@-` 读标准输入。
- 不会自动读标准输入；要从管道传数据，显式写 `--input 名字=@-`。
- `--output 名字` 选择输出哪个块，默认最后一个具名块。
- 密钥类输入（`secret`）从环境变量 `TOOLBOOK_SECRET_<NAME>` 读，或用 `--input` 传。
- 退出码：0 成功；1 某个块出错（stderr 写着「第 n 块（名字，工具）：原因」，`--json` 时看 `error.block` 和 `error.message`）；2 参数或文件错误。

## 没有现成的工具流时：临时写一本

在临时目录写一个 `.md`，开头必须带 front matter `toolbook: 1`，然后 `toolbook run` 它：

````markdown
---
toolbook: 1
---

```text name=header param
Authorization: Bearer eyJ...
```

```jwt-decode name=jwt input=$header
```

```timestamp name=expires input=$jwt.payload.exp
```
````

常用写法：

- `input=$名字.路径` 取上游输出的子字段（`$resp.body`、`$jwt.payload.exp`）。
- `csv dedupe=列 require="列~正则"`、`map rename="旧:新"`、`to-sql table=表 mode=insert|update key=主键`。
- `filter where="duration > 100"`、`pick path="$.spans[*].service"`（JSONPath 要加引号）。
- `js` 块里直接用 `$名字` 当 JS 变量：`$rows.filter(r => r.age > 18)`。
- `http` 块：首行 `GET https://…`，然后请求头、空行、请求体；内容里用 `{{ 名字 }}` 插值。CLI 里会真的发请求，注意副作用。

用户想长期复用的流程，建议保存到 `~/Toolbook/<分类>/<名字>.md`，并把需要替换的块标上 `param`。
