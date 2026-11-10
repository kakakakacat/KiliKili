# 视频详情 golden fixture

- `*.raw.json`：**脱敏后**的 `/x/web-interface/wbi/view` 响应样本。
  其中的 mid、昵称、BV 号、封面地址均为构造值，不对应真实用户或稿件；
  响应里原本就不含凭据（凭据在请求头，不在响应体）。
- `*.normalized.json`：由 Dart 基线实现产出的规范化结果，见
  `tool/fixture-gen/video_detail_fixture.dart`。**不要手工编辑**。

重新生成：

```sh
dart tool/fixture-gen/video_detail_fixture.dart
```

Rust 侧 `tests/video_detail_golden.rs` 用同一份 `*.raw.json` 解析，结果
必须与 `*.normalized.json` 逐字段相同。两侧出现差异时，先判断是基线的
行为变了还是 Rust 实现错了——fixture 是基线的快照，不是裁判。

样本覆盖的维度（新增样本请一并更新此表）：

| 样本 | 覆盖 |
|---|---|
| `single_part` | 最常见形态：单 P、横屏、无合集、无联合投稿 |
| `vertical_multipart` | `rotate=1` 竖屏、多分 P、`desc_v2`、争议提示、联合投稿（含字符串 mid、vip/official 两套键名） |
| `ugc_season` | 合集 sections/episodes/arc、空 `redirect_url`、充电专属、互动视频标记、计数字段缺失 |
