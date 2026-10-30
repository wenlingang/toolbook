---
toolbook: 1
---

# 支付回调验签

核对回调签名：用密钥对请求体做 HMAC-SHA256，看对方用的是 hex 还是 Base64。

密钥不写在文件里：运行前设置环境变量 `TOOLBOOK_SECRET_KEY`，或传入 `--input key=…`。

```text name=body param
{"event":"payment.succeeded","order_id":"ORD20261029001","amount":"128.00","currency":"HKD"}
```

```secret name=key
```

```text name=sig param
Y4ahbDBQIl43PJ5/0LIgNJILoOaYvtbjNCC6DETUEvA=
```

```hmac name=mac data=$body key=$key
```

```js name=match
({ hex: $mac.hex === $sig.trim(), base64: $mac.base64 === $sig.trim() })
```
