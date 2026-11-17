# 批次 5 fixture 来源

2026-11-17 匿名读取官方空间接口：`article_real` 保留实际返回的已消费字段，替换标题、封面与稿件跳转；`audio_empty_real`、`cheese_empty_real` 是匿名空页真实响应。标识与 URL 已脱敏。

`audio_cases`、`cheese_cases` 为按旧 Dart 模型构造的非空/边界样本；匿名空页不足以覆盖条目解析。会员购匿名 POST 实测取得 `shop_empty_real` 空页成功响应（未带登录与签名参数，目标链接已脱敏）；`shop_cases` 为非空手工构造，Phase 2 仍需登录验证非空商品。

图文 gRPC 样本由 `tool/fixture-gen/space_opus_fixture.dart` 使用基线 protobuf 编码器构造，并冻结 Dart 解码投影及 wire SHA-256；不是线上录包。Dart parity 只读 JSON 与字节，不 import lib/。

一次性 Dart parity 共 5 项已通过，随后随旧模型删除；长期保留 Rust golden。
对照覆盖有效标量字段，不声称所有异常输入都保持旧 Widget 输出：缺标题的商品
标签被忽略，优惠后缀的对象/数组不显示 Dart 内部字符串；价格块存在但没有
价格值时不显示字面量 `null` 价格。这些分支刻意避免把缺数据当作文案展示。
