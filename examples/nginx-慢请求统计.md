---
toolbook: 1
---

# 从 Nginx 访问日志里找慢接口

粘一段访问日志（`log_format` 末尾带 `$request_time`），按接口统计次数、最慢耗时和 5xx 次数，健康检查不算。

```text name=log param
10.0.3.21 - - [30/Oct/2026:10:02:11 +0800] "GET /api/orders?page=1 HTTP/1.1" 200 5123 0.084
10.0.3.21 - - [30/Oct/2026:10:02:12 +0800] "POST /api/orders HTTP/1.1" 201 312 1.532
10.0.4.17 - - [30/Oct/2026:10:02:12 +0800] "GET /api/users/42 HTTP/1.1" 200 801 0.012
10.0.4.17 - - [30/Oct/2026:10:02:13 +0800] "POST /api/orders HTTP/1.1" 500 98 3.004
10.0.5.2 - - [30/Oct/2026:10:02:15 +0800] "GET /api/orders?page=2 HTTP/1.1" 200 4980 0.091
10.0.5.2 - - [30/Oct/2026:10:02:16 +0800] "GET /healthz HTTP/1.1" 200 2 0.001
```

```regex name=hits input=$log pattern="\"(\w+) ([^ ?\"]+)[^\"]*\" (\d{3}) \d+ ([\d.]+)"
```

```js name=report
const by = {};
for (const m of $hits.filter(([, path]) => path !== "/healthz")) {
  const [method, path, status, t] = m;
  const k = `${method} ${path}`;
  const s = (by[k] ??= { 次数: 0, 最慢秒: 0, 出错: 0 });
  s.次数 += 1;
  s.最慢秒 = Math.max(s.最慢秒, Number(t));
  if (Number(status) >= 500) s.出错 += 1;
}
Object.entries(by).map(([接口, s]) => ({ 接口, ...s })).sort((a, b) => b.最慢秒 - a.最慢秒)
```
