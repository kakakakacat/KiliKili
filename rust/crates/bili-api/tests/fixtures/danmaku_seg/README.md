# 弹幕分段 golden fixture

弹幕的原始数据是 protobuf 字节，不是可读的 JSON，所以比其他 fixture 多
一层「源描述」：

```text
<name>.source.json      人写的样本描述（唯一手工维护的文件）
  ↓ 基线 protobuf.dart 的编码器
<name>.raw.pb           原始字节，Rust 解码器的输入
  ↓ 基线的解码器 + lib/services/rust/danmaku_normalize.dart
<name>.normalized.json  期望值
```

`.raw.pb` 与 `.normalized.json` **不要手工编辑**。重新生成：

```sh
dart tool/fixture-gen/danmaku_seg_fixture.dart
```

三条测试把这条链钉住：

| 测试 | 钉住什么 |
|---|---|
| `rust/crates/bili-api/tests/danmaku_seg_golden.rs` | Rust 解码结果 == golden |
| `test/services/rust/danmaku_normalize_test.dart` 的「golden 与基线解码结果一致」 | golden == 基线解码结果 |
| 同上的「raw.pb 是基线编码器写出的规范形式」 | 字节是 protobuf.dart 会写出的形式，而非某种线上不会出现的编码 |

中间那条是关键：少了它，Rust 全绿只能证明它与一份来历不明的文件一致。

## 样本

全部为构造值：`mid_hash` 不对应任何真实用户（服务端下发的本就是哈希而
非 mid），`oid`、`id` 亦非真实稿件。弹幕响应里不含凭据。

| 样本 | 覆盖 |
|---|---|
| `typical` | 常见形态：滚动/顶部/底部三种 mode、重复内容（将来合并弹幕用）、同一 100ms 分桶内的多条、一条只有 `content` 的弹幕（proto3 字段缺席取零值） |
| `closed` | `state=1`（视频弹幕已关闭）且无弹幕 |
| `rich` | 高级弹幕（`mode=7`，content 是 JSON）、特殊池、`attr`、`animation`、`extra`、大会员彩色 `colorful=60001`、`dm_from` 非默认值、超出 2^53 的 int64、emoji 与换行 |

未知枚举值与畸形字节**不在这里**：基线的编码器写不出它不认识的枚举，
手工伪造的字节也不构成「基线会产生什么」的证据。那些形态在
`rust/crates/bili-api/src/danmaku.rs` 与 `protobuf.rs` 的单测里构造。
