import 'dart:typed_data';

import 'package:kilikili/grpc/bilibili/community/service/dm/v1.pb.dart';
import 'package:kilikili/grpc/grpc_req.dart';
import 'package:kilikili/grpc/url.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models_new/video/video_play_info/subtitle.dart';
import 'package:kilikili/services/rust/rust_dm_view.dart';
import 'package:kilikili/rust/api/grpc.dart' as rust_api;
import 'package:kilikili/services/rust/rust_grpc.dart';
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
  /// 传输与解帧在 Rust 工作线程上完成，不再让两端各处理一半响应。
  static Future<LoadingState<Uint8List>> dmSegMobileRaw({
    required int cid,
    required int segmentIndex,
    int type = 1,
  }) {
    return _fetchSegment(cid, segmentIndex, type);
  }

  static Future<LoadingState<Uint8List>> _fetchSegment(
    int cid,
    int segmentIndex,
    int type,
  ) async {
    try {
      final result = await rust_api.fetchDmSegment(
        context: RustGrpc.context(),
        cid: cid,
        segmentIndex: segmentIndex,
        kind: type,
      );
      final data = result.data;
      if (data == null) {
        return Error(result.errorDetail ?? '弹幕请求失败', code: result.errorCode);
      }
      return Success<Uint8List>(Uint8List.fromList(data));
    } catch (_) {
      return const Error('Rust 弹幕桥接不可用');
    }
  }

  /// 请求、解帧、字幕解析与排序均由 Rust 完成；空列表表示没有字幕。
  static Future<LoadingState<List<Subtitle>>> dmViewSubtitles(
    int aid,
    int cid,
  ) => RustDmView.fetchSubtitles(aid, cid);
}
