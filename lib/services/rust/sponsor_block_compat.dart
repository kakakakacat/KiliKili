import 'package:kilikili/models_new/sponsor_block/segment_item.dart';
import 'package:kilikili/services/rust/rust_sponsor_block.dart' show SkipSegment;

/// typed 直连的过渡适配：Rust 的 [SkipSegment] → 基线的 `SegmentItemModel`。
///
/// ## 为什么还需要它
///
/// `SegmentItemModel` 不只被网络路径使用，还有三个尚未迁移的生产者：
/// PGC 的 `clip_info_list`（`PlayUrlModel.clipInfoList`）、下载条目的本地
/// 缓存（`BiliDownloadEntryInfo.segments`）、以及 SponsorBlock 的写接口
/// （提交片段后服务端回传片段列表）。只要它们还在产出这个类型，它就删不掉。
///
/// 所以本次迁移的边界是**网络边界**：跨 FFI 的形状不再是 JSON 字符串、
/// 不再有 normalize/denormalize 双写；而运行时共享模型要等它的其余生产者
/// 一起迁移。写接口迁移（需要 ADR-013 / #50）时，本文件应当整体删除。
///
/// 这条经验对其他切片同样适用：**先看共享运行时模型还有几个生产者**，
/// 只迁其中一个的话，模型删不掉是正常的，不要为此把别的切片拖进来。
extension RustSkipSegmentCompat on SkipSegment {
  SegmentItemModel toItemModel() => SegmentItemModel(
    cid: cid,
    category: category,
    actionType: actionType,
    segment: [startMs, endMs],
    uuid: uuid,
    videoDuration: videoDurationMs,
    votes: votes,
  );
}
