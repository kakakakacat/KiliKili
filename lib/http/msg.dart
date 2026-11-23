import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/init.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/utils/accounts.dart';

/// 这个文件**只剩一条**，而且它不属于消息域。
///
/// 消息中心与私信设置的 19 条已随 S12（#63）迁到 Rust，两条图片上传
/// （`uploadImage` / `uploadBfs`）随 ADR-015 / #193 迁走，见
/// `lib/services/rust/rust_upload.dart`；`models_new/upload_bfs/` 也一并删了。
///
/// `removeDynamic` 属于**动态域**，归 S5 续单 **#150** 的
/// `lib/http/dynamics.dart` 一起处理。本片不去动别的切片的文件——它迁走
/// 之后这个文件整个删掉。
abstract final class MsgHttp {
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
