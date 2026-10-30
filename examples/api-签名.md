---
toolbook: 1
---

# 生成开放平台的请求签名

很多开放平台的签名规则是：参数按名字排序，拼成 `k=v&k=v`，再用密钥做 HMAC-SHA256。这里把规则写成步骤，联调时对一对自己算的签名。

密钥不写在文件里：运行前设置环境变量 `TOOLBOOK_SECRET_APP_SECRET`，或在应用里输入。

```json name=params param
{ "app_id": "a1b2c3", "timestamp": "1761789600", "nonce": "8f14e45f", "order_id": "ORD-1001", "amount": "128.00" }
```

```js name=canonical
Object.keys($params).sort().map(k => `${k}=${$params[k]}`).join("&")
```

```secret name=app_secret
```

```hmac name=sign data=$canonical key=$app_secret
```
