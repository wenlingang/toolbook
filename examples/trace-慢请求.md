---
toolbook: 1
---

# 下单慢请求

粘贴一条 trace，列出耗时超过 100ms 的 span，找出真正慢的那一个。

```json name=trace param
{"traceId":"4bf92f3577b34da6a3ce929d0e0e4736","spans":[
  {"service":"gateway","name":"POST /v1/orders","start":0,"duration":312},
  {"service":"order-api","name":"CreateOrder","start":8,"duration":290},
  {"service":"risk-check","name":"CheckLimit","start":20,"duration":44},
  {"service":"quote-svc","name":"GetQuote","start":66,"duration":52},
  {"service":"mysql","name":"INSERT orders","start":122,"duration":164},
  {"service":"redis","name":"SET order:cache","start":288,"duration":6}
]}
```

```filter name=slow input=$trace.spans where="duration > 100"
```

```js name=summary
$slow.map(s => `${s.service} ${s.name} ${s.duration}ms`).join("\n")
```
