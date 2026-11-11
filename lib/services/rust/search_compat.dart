import 'package:kilikili/models/search/result.dart';
import 'package:kilikili/rust/api/search.dart' show SearchVideoItem;

/// typed 直连的过渡适配：Rust 的 [SearchVideoItem] → 基线的
/// [SearchVideoItemModel]。
///
/// ## 为什么还需要它
///
/// [SearchVideoItemModel] 继承 `HorizontalVideoModel`，而那个共享运行时
/// 模型还有三个**尚未迁移**的生产者：
///
/// - `HotVideoItemModel`（热门/推荐列表，`lib/http/video.dart`）
/// - `VListItemModel`（用户空间投稿搜索，S6 / #56）
/// - `SeasonArchive`（用户空间合集，S6 / #56）
///
/// 按本仓库在 S13 定下的边界（ADR-011 决策六），迁移的边界是**网络
/// 边界**：跨 FFI 的形状不再是 JSON、Dart 侧不再有第二份解析规则；而
/// 共享的运行时模型要等它的**所有**生产者迁完才能删。把
/// `HorizontalVideoModel` 在本切片里拆掉，会连带改写那三个生产者和按
/// 基类接收的通用列表组件——那是 S6 与推荐流迁移的工作，不是搜索的。
///
/// 这些生产者都迁完之后，本文件应当整体删除。
extension RustSearchVideoCompat on SearchVideoItem {
  SearchVideoItemModel toItemModel() {
    final model = SearchVideoItemModel()
      ..id = id
      ..aid = aid
      ..bvid = bvid
      ..arcurl = arcurl
      // Rust 已经把 <em> 标记拆好并解过实体，Dart 侧不再跑一遍 Em.regTitle。
      ..titleList = titleSpans
          .map((s) => (isEm: s.isEm, text: s.text))
          .toList()
      ..title = title
      ..desc = description
      ..cover = pic
      ..pubdate = pubdate
      ..ctime = senddate
      ..duration = durationSecs
      ..tag = tag
      ..owner = (SearchOwner()
        ..mid = owner.mid
        ..name = owner.name
        ..face = owner.face)
      ..stat = (SearchStat()
        ..view = stat.view
        ..danmu = stat.danmaku
        ..favorite = stat.favorite
        ..reply = stat.reply
        ..like = stat.like);

    // 角标文案与 isPugv/isLive 是**界面**，所以留在这里而不是 Rust
    // （ADR-011 决策二）。Rust 只带回 `itemType` 与 `isUnionVideo` 两个
    // 协议字段。
    switch (itemType) {
      case 'ketang':
        model
          ..badge = '课堂'
          ..isPugv = true;
      case 'live_room':
        model
          ..badge = '直播'
          ..isLive = true
          ..roomId = roomid;
      default:
        if (isUnionVideo == 1) model.badge = '合作';
    }
    return model;
  }
}

extension RustSearchVideoListCompat on Iterable<SearchVideoItem> {
  List<SearchVideoItemModel> toItemModels() =>
      map((e) => e.toItemModel()).toList();
}

/// 富文本段落的纯文本形式。
///
/// Rust 已经随条目一起给了拼好的 `title`，这个扩展只给**没有**现成纯文本
/// 字段的地方用（比如把富文本标题塞进图片保存对话框）。
extension TitleSpansText on Iterable<({bool isEm, String text})> {
  String get plainText => map((e) => e.text).join();
}
