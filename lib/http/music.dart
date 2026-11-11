import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/init.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:dio/dio.dart';

/// 音乐页的**写操作**。两条只读接口（BGM 详情、相关推荐）已迁到
/// `lib/services/rust/rust_misc.dart`（S15 / issue #66）；点「想听」等
/// ADR-013（issue #50）。
abstract final class MusicHttp {
  static Future<LoadingState<void>> wishUpdate(
    String musicId,
    bool hasLike,
  ) async {
    final res = await Request().post(
      Api.wishUpdate,
      data: {
        'music_id': musicId,
        'state': hasLike ? 2 : 1,
        'csrf': Accounts.main.csrf,
      },
      options: Options(contentType: Headers.formUrlEncodedContentType),
    );
    if (res.data['code'] == 0) {
      return const Success(null);
    } else {
      return Error(res.data['message']);
    }
  }
}
