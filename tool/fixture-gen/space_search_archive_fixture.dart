import 'dart:convert';
import 'dart:io';

import 'package:kilikili/grpc/bilibili/app/archive/v1.pb.dart' as archive;
import 'package:kilikili/grpc/bilibili/app/interfaces/v1.pb.dart'
    show Arc, SearchArchiveReply;
import 'package:fixnum/fixnum.dart';

/// 空间内 gRPC 投稿搜索的 golden fixture 生成器（S6 #56）。
///
/// 链条与 `dm_view_fixture.dart` 相同：
///
/// ```text
/// <name>.source.json   人写的样本描述
///   ↓ 基线的 protobuf 编码器
/// <name>.raw.pb        原始字节，Rust 解码器的输入
/// ```
///
/// 这条接口**抓不到真实响应**：它走 app 的 gRPC 网关，要设备指纹与
/// access_key。用基线自己的编码器写出规范字节，至少保证两端对同一串
/// 字节的理解一致；对协议的理解是否正确，按 #67 留到 Phase 2 真机验。
///
/// 源描述里**刻意**放了几个没有消费方的字段（`view_content`、`icon_type`、
/// `badges`、`is_fold`、`videos`、`type_name`……）：它们会进字节，Rust 必须
/// 跳过而不是报错。
///
/// 用法: fvm dart run tool/fixture-gen/space_search_archive_fixture.dart
const fixtureDir = 'rust/crates/bili-api/tests/fixtures/space_grpc';

void main() {
  final sources =
      Directory(fixtureDir)
          .listSync()
          .whereType<File>()
          .where((f) => f.path.endsWith('.source.json'))
          .toList()
        ..sort((a, b) => a.path.compareTo(b.path));
  if (sources.isEmpty) {
    stderr.writeln('没有 SearchArchive fixture');
    exit(1);
  }

  for (final source in sources) {
    final spec = jsonDecode(source.readAsStringSync()) as Map<String, dynamic>;
    final reply = _replyFromSpec(spec);
    final stem = source.path.replaceFirst('.source.json', '');
    File('$stem.raw.pb').writeAsBytesSync(reply.writeToBuffer());
  }
}

SearchArchiveReply _replyFromSpec(Map<String, dynamic> spec) {
  final reply = SearchArchiveReply();
  if (spec['total'] case final int v) reply.total = Int64(v);
  for (final item in (spec['archives'] as List? ?? const [])) {
    reply.archives.add(_arcFromSpec(item as Map<String, dynamic>));
  }
  return reply;
}

/// 源描述里**没有**的键一律不设置，由 proto3 取零值。
Arc _arcFromSpec(Map<String, dynamic> spec) {
  final arc = Arc();
  if (spec['uri'] case final String v) arc.uri = v;
  if (spec['view_content'] case final String v) arc.viewContent = v;
  if (spec['icon_type'] case final int v) arc.iconType = Int64(v);
  if (spec['cover_icon'] case final String v) arc.coverIcon = v;
  if (spec['is_fold'] case final bool v) arc.isFold = v;
  if (spec['is_pugv'] case final bool v) arc.isPugv = v;
  if (spec['publish_time_text'] case final String v) arc.publishTimeText = v;
  if (spec['badges'] case final List v) arc.badges.addAll(v.cast<String>());
  if (spec['is_oneself'] case final bool v) arc.isOneself = v;
  if (spec['archive'] case final Map<String, dynamic> v) {
    arc.archive = _archiveFromSpec(v);
  }
  return arc;
}

archive.Arc _archiveFromSpec(Map<String, dynamic> spec) {
  final inner = archive.Arc();
  if (spec['aid'] case final int v) inner.aid = Int64(v);
  if (spec['videos'] case final int v) inner.videos = Int64(v);
  if (spec['type_id'] case final int v) inner.typeId = v;
  if (spec['type_name'] case final String v) inner.typeName = v;
  if (spec['pic'] case final String v) inner.pic = v;
  if (spec['title'] case final String v) inner.title = v;
  if (spec['pubdate'] case final int v) inner.pubdate = Int64(v);
  if (spec['ctime'] case final int v) inner.ctime = Int64(v);
  if (spec['desc'] case final String v) inner.desc = v;
  if (spec['duration'] case final int v) inner.duration = Int64(v);
  if (spec['redirect_url'] case final String v) inner.redirectUrl = v;
  if (spec['first_cid'] case final int v) inner.firstCid = Int64(v);
  if (spec['author'] case final Map<String, dynamic> v) {
    inner.author = archive.Author();
    if (v['mid'] case final int mid) inner.author.mid = Int64(mid);
    if (v['name'] case final String name) inner.author.name = name;
    if (v['face'] case final String face) inner.author.face = face;
  }
  if (spec['stat'] case final Map<String, dynamic> v) {
    inner.stat = archive.Stat();
    if (v['aid'] case final int aid) inner.stat.aid = Int64(aid);
    if (v['view'] case final int view) inner.stat.view = view;
    if (v['danmaku'] case final int danmaku) inner.stat.danmaku = danmaku;
    if (v['reply'] case final int reply) inner.stat.reply = reply;
  }
  if (spec['dimension'] case final Map<String, dynamic> v) {
    inner.dimension = archive.Dimension();
    if (v['width'] case final int w) inner.dimension.width = Int64(w);
    if (v['height'] case final int h) inner.dimension.height = Int64(h);
    if (v['rotate'] case final int r) inner.dimension.rotate = Int64(r);
  }
  return inner;
}
