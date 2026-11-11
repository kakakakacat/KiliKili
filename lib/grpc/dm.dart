import 'dart:typed_data';

import 'package:kilikili/grpc/bilibili/community/service/dm/v1.pb.dart';
import 'package:kilikili/grpc/grpc_req.dart';
import 'package:kilikili/grpc/url.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models_new/video/video_play_info/subtitle.dart';
import 'package:fixnum/fixnum.dart';

abstract final class DmGrpc {
  static Future<LoadingState<DmSegMobileReply>> dmSegMobile({
    required int cid,
    required int segmentIndex,
    int type = 1,
  }) {
    return GrpcReq.request(
      GrpcUrl.dmSegMobile,
      DmSegMobileReq(
        oid: Int64(cid),
        segmentIndex: Int64(segmentIndex),
        type: type,
      ),
      DmSegMobileReply.fromBuffer,
      isolate: true,
    );
  }

  /// 同 [dmSegMobile]，但只取回解压后的 protobuf 字节，由 Rust 引擎解析。
  ///
  /// 不走 `isolate: true`：那个开关是给 Dart 侧解析用的（基线在响应超过
  /// 256 KiB 时把 `fromBuffer` 搬去 isolate），而这里根本不在 Dart 解析。
  /// 解析在 Rust 的工作线程上，比 isolate 还少一次消息拷贝。
  static Future<LoadingState<Uint8List>> dmSegMobileRaw({
    required int cid,
    required int segmentIndex,
    int type = 1,
  }) {
    return GrpcReq.requestRaw(
      GrpcUrl.dmSegMobile,
      DmSegMobileReq(
        oid: Int64(cid),
        segmentIndex: Int64(segmentIndex),
        type: type,
      ),
    );
  }

  static Future<LoadingState<DmViewReply>> dmView(int aid, int cid) {
    return GrpcReq.request(
      GrpcUrl.dmView,
      DmViewReq(pid: Int64(aid), oid: Int64(cid), type: 1),
      DmViewReply.fromBuffer,
    );
  }

  static List<Subtitle>? dmViewToSubtitle(DmViewReply? dmView) {
    if (dmView == null) return null;

    if (dmView.hasSubtitle()) {
      final subtitles = dmView.subtitle.subtitles;
      if (subtitles.isNotEmpty) {
        return subtitles
            .map(
              (e) => Subtitle(
                id: e.id.toInt(),
                lan: e.lan,
                lanDoc: e.lanDoc,
                type: e.type.value,
                subtitleUrl: e.subtitleUrl,
                isAi: e.type == .AI,
              ),
            )
            .toList()
          ..sort();
      }
    }

    return null;
  }
}
