---
toolbook: 1
---

# 查一个 GitHub 仓库的近况

调公开的 GitHub API，看仓库的星数、最近一次推送和最新版本。HTTP 块不会自动发请求：在应用里按 ⌘↵ 运行。

```text name=repo param
rust-lang/rust
```

```http name=info
GET https://api.github.com/repos/{{ repo }}
Accept: application/vnd.github+json
User-Agent: toolbook
```

```http name=release
GET https://api.github.com/repos/{{ repo }}/releases/latest
Accept: application/vnd.github+json
User-Agent: toolbook
```

```js name=summary
({
  仓库: $info.body.full_name,
  星数: $info.body.stargazers_count,
  最近推送: $info.body.pushed_at,
  最新版本: $release.body.tag_name,
})
```
