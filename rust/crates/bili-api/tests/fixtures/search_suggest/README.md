# 搜索建议 fixture

`*.raw.json` 是合成的脱敏响应；`*.normalized.json` 由现有 Dart
`SearchSuggestModel.fromJson` 生成，作为 Rust 解析器的对照基线。

在仓库根目录运行：

```sh
fvm dart tool/fixture-gen/search_suggest_fixture.dart
```

`rich` 覆盖中文建议、富文本标记和额外字段；`empty` 覆盖缺失的 `tag`。
