import 'package:kilikili/grpc/bilibili/app/dynamic/v2.pb.dart'
    show
        RepostListReq,
        RepostListRsp,
        RepostType;
import 'package:kilikili/grpc/grpc_req.dart';
import 'package:kilikili/grpc/url.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:fixnum/fixnum.dart';

abstract final class DynGrpc {
  // static Future dynSpace({
  //   required int uid,
  //   required int page,
  // }) {
  //   return _request(
  //     GrpcUrl.dynSpace,
  //     DynSpaceReq(
  //       hostUid: Int64(uid),
  //       localTime: 8,
  //       page: Int64(page),
  //       from: 'space',
  //     ),
  //     DynSpaceRsp.fromBuffer,
  //   );
  // }

  static Future<LoadingState<RepostListRsp>> repostList({
    required String dynamicId,
    Int64? dynType,
    Int64? rid,
    String? offset,
    String? from,
    RepostType repostType = .repost_general,
  }) {
    return GrpcReq.request(
      GrpcUrl.repostList,
      RepostListReq(
        dynamicId: dynamicId,
        dynType: dynType,
        rid: rid,
        offset: offset,
        repostType: repostType,
      ),
      RepostListRsp.fromBuffer,
    );
  }
}
