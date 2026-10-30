---
toolbook: 1
---

# 看 Kubernetes Secret 里存了什么

`kubectl get secret app-env -o json` 的输出里，`data` 下的值都是 Base64。粘进来，逐个解出明文；换一个 Secret 时，改下面几块的字段名就行。

```json name=secret param
{
  "apiVersion": "v1",
  "kind": "Secret",
  "metadata": { "name": "app-env", "namespace": "staging" },
  "type": "Opaque",
  "data": {
    "DB_HOST": "bXlzcWwuc3RhZ2luZy5zdmM=",
    "DB_USER": "YXBw",
    "REDIS_URL": "cmVkaXM6Ly9yZWRpcy5zdGFnaW5nLnN2Yzo2Mzc5LzA="
  }
}
```

```base64 name=db_host input=$secret.data.DB_HOST
```

```base64 name=db_user input=$secret.data.DB_USER
```

```base64 name=redis_url input=$secret.data.REDIS_URL
```
