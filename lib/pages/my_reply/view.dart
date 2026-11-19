import 'dart:typed_data';

import 'package:kilikili/common/widgets/dialog/dialog.dart';
import 'package:kilikili/common/widgets/dialog/export_import.dart';
import 'package:kilikili/common/widgets/loading_widget/http_error.dart';
import 'package:kilikili/common/widgets/scaffold/simple_scaffold.dart';
import 'package:kilikili/common/widgets/view_sliver_safe_area.dart';
import 'package:kilikili/pages/article/view.dart';
import 'package:kilikili/pages/video/reply/widgets/reply_item_grpc.dart';
import 'package:kilikili/pages/video/reply_reply/view.dart';
import 'package:kilikili/services/rust/rust_reply_grpc.dart';
import 'package:kilikili/utils/app_scheme.dart';
import 'package:kilikili/utils/bili_utils.dart';
import 'package:kilikili/utils/id_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/reply_utils.dart';
import 'package:kilikili/utils/storage.dart';
import 'package:kilikili/utils/storage_pref.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:kilikili/utils/waterfall.dart';
import 'package:flutter/foundation.dart' show kDebugMode;
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:flutter_staggered_grid_view/flutter_staggered_grid_view.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class MyReply extends StatefulWidget {
  const MyReply({super.key});

  @override
  State<MyReply> createState() => _MyReplyState();

  /// 把一份导出文件变成可以直接写进盒子的 `键 → 字节`。
  ///
  /// 格式的读写在 Rust（`bili_api::reply_grpc_store`）：老的 proto3 JSON
  /// 导出文件与新格式都认。拿不到结果时返回 null，调用点据此报错而不是
  /// 静默写入半份数据。
  static Map<String, Uint8List>? json2ReplyBytes(List<dynamic> list) =>
      RustReplyGrpc.importStored(Utils.jsonEncoder.convert(list));
}

class _MyReplyState extends State<MyReply> with DynMixin {
  final List<ReplyEntity> _replies = <ReplyEntity>[];

  @override
  void initState() {
    super.initState();
    _initReply();
  }

  void _initReply() {
    _replies
      // 解不开的条目直接跳过：一条坏数据不该让整页打不开。
      ..assignAll(GStorage.reply!.values.map(RustReplyGrpc.decodeStored).nonNulls)
      ..sort((a, b) => b.ctime.compareTo(a.ctime)); // rpid not aligned;
  }

  @override
  Widget build(BuildContext context) {
    return SimpleScaffold(
      appBar: AppBar(
        title: const Text('我的评论'),
        actions: [
          if (kDebugMode) ...[
            IconButton(
              tooltip: 'Clear',
              onPressed: () => showConfirmDialog(
                context: context,
                title: const Text('Clear Local Storage?'),
                onConfirm: () {
                  GStorage.reply!.clear();
                  _replies.clear();
                  setState(() {});
                },
              ),
              icon: const Icon(Icons.clear_all),
            ),
            IconButton(
              tooltip: 'Refresh',
              onPressed: () => setState(_initReply),
              icon: const Icon(Icons.refresh),
            ),
          ],
          IconButton(
            tooltip: '导出',
            onPressed: _showExportDialog,
            icon: const Icon(Icons.file_upload_outlined),
          ),
          IconButton(
            tooltip: '导入',
            onPressed: _showImportDialog,
            icon: const Icon(Icons.file_download_outlined),
          ),
          const SizedBox(width: 6),
        ],
      ),
      body: CustomScrollView(
        physics: const AlwaysScrollableScrollPhysics(),
        slivers: [
          _replies.isNotEmpty
              ? ViewSliverSafeArea(
                  sliver: SliverMasonryGrid(
                    crossAxisSpacing: 4,
                    gridDelegate: dynGridDelegate,
                    delegate: SliverChildBuilderDelegate(
                      childCount: _replies.length,
                      (context, index) => ReplyItemGrpc(
                        replyLevel: 0,
                        needDivider: false,
                        replyItem: _replies[index],
                        replyReply: _replyReply,
                        onDelete: (_, _) => _onDelete(index),
                        onCheckReply: _onCheckReply,
                      ),
                    ),
                  ),
                )
              : const HttpError(),
        ],
      ),
    );
  }

  void _replyReply(ReplyEntity replyInfo, int? rpid) {
    int? rootRpId, secondRpId;
    final id = replyInfo.id;
    if (replyInfo.root != 0) {
      rootRpId = replyInfo.root;
      secondRpId = id;
    } else {
      rootRpId = id;
    }
    final type = replyInfo.kind;
    switch (type) {
      case 1:
        PiliScheme.videoPush(
          replyInfo.oid,
          null,
          rootRpId: rootRpId,
          secondRpId: secondRpId,
        );
      case 12:
        ArticlePage.toArticlePage(
          id: replyInfo.oid.toString(),
          type: .read,
          rootRpId: rootRpId,
          secondRpId: secondRpId,
        );
      case _:
        if (BiliUtils.isDynOrArticle(type)) {
          PageUtils.pushDynFromId(
            rid: replyInfo.oid.toString(),
            type: type,
            rootRpId: rootRpId,
            secondRpId: secondRpId,
          );
        } else {
          // TODO:
          VideoReplyReplyPanel.toReply(
            oid: replyInfo.oid,
            rootId: rootRpId,
            rpId: secondRpId,
            type: type,
          );
        }
    }
  }

  void _onDelete(int index) {
    _replies.removeAt(index);
    setState(() {});
  }

  void _onCheckReply(ReplyEntity replyInfo) {
    final oid = replyInfo.oid;
    ReplyUtils.onCheckReply(
      replyInfo: replyInfo,
      biliSendCommAntifraud: Pref.biliSendCommAntifraud,
      sourceId: switch (oid) {
        1 => IdUtils.av2bv(oid),
        _ => oid.toString(),
      },
      isManual: true,
    );
  }

  /// 导出失败时抛出。基线用 protobuf 生成类做这件事，不会失败；现在格式
  /// 在 Rust，盒子里有一条读不出来的数据就会整个失败——那比导出一份少了
  /// 几条的存档好。两个入口各自接住它（[_runExport]）。
  String _onExport() {
    final json = RustReplyGrpc.exportStored(GStorage.reply!.values.toList());
    if (json == null) throw StateError('评论存档导出失败');
    return json;
  }

  void _runExport(VoidCallback export) {
    try {
      export();
    } catch (e) {
      SmartDialog.showToast('导出失败：$e');
    }
  }

  void _showExportDialog() {
    const style = TextStyle(fontSize: 14);
    showDialog(
      context: context,
      builder: (context) => SimpleDialog(
        clipBehavior: .hardEdge,
        contentPadding: const .symmetric(vertical: 12),
        children: [
          ListTile(
            dense: true,
            title: const Text('导出至剪贴板', style: style),
            onTap: () {
              Get.back();
              _runExport(() => exportToClipBoard(onExport: _onExport));
            },
          ),
          ListTile(
            dense: true,
            title: const Text('导出文件至本地', style: style),
            onTap: () {
              Get.back();
              _runExport(
                () => exportToLocalFile(
                  onExport: _onExport,
                  localFileName: () => 'reply',
                ),
              );
            },
          ),
        ],
      ),
    );
  }

  Future<void> _onImport(List<dynamic> list) async {
    final entries = MyReply.json2ReplyBytes(list);
    if (entries == null) throw StateError('评论存档不是合法的格式');
    await GStorage.reply!.putAll(entries);
    if (mounted) {
      _initReply();
      setState(() {});
    }
  }

  void _showImportDialog() {
    const style = TextStyle(fontSize: 14);
    showDialog(
      context: context,
      builder: (context) => SimpleDialog(
        clipBehavior: .hardEdge,
        contentPadding: const .symmetric(vertical: 12),
        children: [
          ListTile(
            dense: true,
            title: const Text('从剪贴板导入', style: style),
            onTap: () {
              Get.back();
              importFromClipBoard<List<dynamic>>(
                context,
                title: '评论',
                onExport: _onExport,
                onImport: _onImport,
                showConfirmDialog: false,
              );
            },
          ),
          ListTile(
            dense: true,
            title: const Text('从本地文件导入', style: style),
            onTap: () {
              Get.back();
              importFromLocalFile<List<dynamic>>(onImport: _onImport);
            },
          ),
        ],
      ),
    );
  }
}
