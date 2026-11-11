import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/init.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/services/rust/rust_misc.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:dio/dio.dart';

/// 弹幕屏蔽规则的**写操作**。只读的那条（拉取云端规则）已迁到
/// `lib/services/rust/rust_misc.dart`（S15 / issue #66）；这两个写操作等
/// ADR-013（issue #50）。
abstract final class DanmakuFilterHttp {
  static Future<LoadingState<void>> danmakuFilterDel({required int ids}) async {
    final res = await Request().post(
      Api.danmakuFilterDel,
      data: {
        'ids': ids,
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

  /// 新增一条规则。返回的是**Rust 侧的 typed 结构**：规则只剩这一个类型，
  /// 读路径已经迁走，这里再留一个等价的 Dart 模型只会让两边各自漂移。
  /// FRB 生成的 Dart 类是普通数据类，可以直接构造。
  static Future<LoadingState<FilterRule>> danmakuFilterAdd({
    required String filter,
    required int type,
  }) async {
    final res = await Request().post(
      Api.danmakuFilterAdd,
      data: {
        'type': type,
        'filter': filter,
        'csrf': Accounts.main.csrf,
      },
      options: Options(contentType: Headers.formUrlEncodedContentType),
    );
    if (res.data['code'] == 0) {
      final data = res.data['data'];
      return Success(
        FilterRule(
          id: data['id'],
          kind: data['type'],
          filter: data['filter'],
        ),
      );
    } else {
      return Error(res.data['message']);
    }
  }
}
