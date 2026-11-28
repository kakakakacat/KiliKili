abstract final class GrpcUrl {
  // static const playerOnline =
  //     '/bilibili.app.playeronline.v1.PlayerOnline/PlayerOnline';
  // static const popular = '/bilibili.app.show.v1.Popular/Index';

  // dynamic
  static const dynV1 = '/bilibili.app.dynamic.v1.Dynamic';
  static const dynV2 = '/bilibili.app.dynamic.v2.Dynamic';
  static const opusV2 = '/bilibili.app.dynamic.v2.Opus';
  static const dynRed = '$dynV1/DynRed';
  static const opusSpaceFlow = '$opusV2/OpusSpaceFlow';
  static const opusDetail = '$opusV2/OpusDetail';
  // static const dynSpace = '$dynV2/DynSpace';
  static const likeList = '$dynV2/LikeList';
  static const repostList = '$dynV2/RepostList';

  // danmaku
  static const dmSegMobile = '/bilibili.community.service.dm.v1.DM/DmSegMobile';
  static const dmView = '/bilibili.community.service.dm.v1.DM/DmView';

  // 评论的六条路径随取数一并迁到 Rust（`bili_protocol::grpc` 的
  // `REPLY_*`，见 issue #176），这里不再保留第二份。

  // IM 的 17 条路径随取数一并迁到 Rust（`bili_protocol::grpc` 的
  // `IM_*`，见 issue #63），这里不再保留第二份。

  // view
  static const viewunite = '/bilibili.app.viewunite.v1.View';
  static const view = '$viewunite/View';

  // 音频的五条路径迁到 `bili_protocol::grpc`，见 issue #203。
}
