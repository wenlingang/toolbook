---
toolbook: 1
---

# 用表格批量改库存

运营给了一张「商品 ID + 新库存」的表，生成 UPDATE 语句，先在测试库跑一遍。

```csv name=rows require="sku~^SKU-\d+$" param
sku,stock,price
SKU-1001,25,99.00
SKU-1002,0,129.00
SKU-1003,180,59.90
```

```to-sql name=sql input=$rows table=products mode=update key=sku
```
