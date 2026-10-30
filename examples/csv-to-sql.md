---
toolbook: 1
---

# 白名单 CSV 转 SQL

把运营给的白名单 CSV 去重、校验手机号、改成英文字段名，生成 INSERT 语句。

```csv name=rows dedupe=手机 require="手机~^1\d{10}$" param
姓名,手机,城市
张三,13800138000,深圳
李四,1390013,广州
王五,13700137000,香港
张三,13800138000,深圳
赵六,15900159000,上海
```

```map name=clean input=$rows rename="姓名:name,手机:phone,城市:city"
```

```to-json name=json input=$clean
```

```to-sql name=sql input=$clean table=campaign_whitelist
```
