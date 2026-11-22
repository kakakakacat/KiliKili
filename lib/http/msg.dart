import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/init.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models_new/upload_bfs/data.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:dio/dio.dart';

/// 这个文件**只剩三条**，而且没有一条属于消息域。
///
/// 消息中心与私信设置的 19 条接口已随 S12（issue #63）迁到 Rust，
/// 见 `lib/services/rust/rust_msg.dart`；对应的 `lib/models_new/msg/**`
/// 与 `lib/models_new/{single_unread,msgfeed_unread}/` 也在同一个 PR 里
/// 删掉了（ADR-011 决策四：不留旧实现、不设降级开关）。
///
/// 剩下这三条留在 Dart 是**刻意的**，各有理由，别顺手一起迁：
///
/// - [uploadImage] / [uploadBfs] 是 multipart 文件上传，还要带
///   `CancelToken`。ADR-013 决策一只给传输层定了 `Form` 与 `Json` 两种
///   请求体，**multipart 与上传取消是一项还没有决策的传输能力**，该有
///   自己的 ADR，不该夹带在消息这一片里。而且它们俩服务的是动态发布与
///   收藏夹封面，不是私信。
/// - [removeDynamic] 属于动态域，归 S5 续单（#150）的
///   `lib/http/dynamics.dart`。本片不去动别的切片的文件。
///
/// 这两条的去向见 **issue #193**：multipart、上传取消与进度回调加起来是
/// 一份 ADR 的量，迁完之后这个文件整个删掉。
///
/// 随本片一并删掉的还有两条**全仓库没有调用点**的：`createTextDynamic`
/// 与 `setTop`（会话置顶早已改走 gRPC 的 `pinSession`）。
abstract final class MsgHttp {
  static Future<LoadingState<Map>> uploadImage({
    required dynamic path,
    required String bucket,
    required String dir,
  }) async {
    final res = await Request().post(
      Api.uploadImage,
      data: FormData.fromMap({
        'bucket': bucket,
        'file': await MultipartFile.fromFile(path),
        'dir': dir,
        'csrf': Accounts.main.csrf,
      }),
    );
    if (res.data['code'] == 0) {
      return Success(res.data['data']);
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<UploadBfsResData>> uploadBfs({
    required String path,
    String? category,
    String? biz,
    CancelToken? cancelToken,
  }) async {
    final res = await Request().post(
      Api.uploadBfs,
      data: FormData.fromMap({
        'file_up': await MultipartFile.fromFile(path),
        'category': ?category,
        'biz': ?biz,
        'csrf': Accounts.main.csrf,
      }),
      cancelToken: cancelToken,
    );
    if (res.data['code'] == 0) {
      return Success(UploadBfsResData.fromJson(res.data['data']));
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<void>> removeDynamic({
    required Object dynIdStr,
    Object? dynType,
    Object? ridStr,
  }) async {
    final res = await Request().post(
      Api.removeDynamic,
      queryParameters: {
        'platform': 'web',
        'csrf': Accounts.main.csrf,
      },
      data: {
        "dyn_id_str": dynIdStr,
        "dyn_type": ?dynType,
        "rid_str": ?ridStr,
      },
    );
    if (res.data['code'] == 0) {
      return const Success(null);
    } else {
      return Error(res.data['message']);
    }
  }
}
