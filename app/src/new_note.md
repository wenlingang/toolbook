---
toolbook: 1
---

# {{name}}

这是一本新的 Toolbook 笔记：说明文字和工具块写在一起，工具块的结果会显示在它下面。

## 基础用法

下面四个块是一个小例子。第一块是输入，后面的块用 `$名字` 取前面块的结果，一步步处理。试着改一下第一块里的时间，后面的结果会跟着变。

```text name=input
{"user":"wen","role":"trader","exp":1761703600}
```

```json name=data input=$input
```

```pick name=exp input=$data path=exp
```

```timestamp name=when input=$exp
```

## 小提示

- 在新的一行输入 `/`，可以插入工具：JSON、CSV、SQL、JWT、Base64、Hash、HMAC、正则、Diff、HTTP、JS 表达式……
- 每个块的第一行可以直接改：工具名、`name=` 名字，以及 `input=$名字` 这类参数。
- HTTP 块不会自动发请求，按 ⌘↵ 运行全部。
- 按 ⌘/ 切换到 Markdown 视图，可以直接复制整篇笔记。

看懂之后，可以删掉这个例子，开始写自己的。
