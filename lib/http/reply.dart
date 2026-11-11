import 'package:kilikili/common/constants.dart';
import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/init.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/accounts/account.dart';
import 'package:dio/dio.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';

/// 评论的**写操作**。只读部分（评论列表、楼中楼、评论区开关、表情包）已
/// 迁到 `lib/services/rust/rust_reply.dart`（S1 / issue #51）；这里剩下的
/// 五个写操作等 ADR-013（issue #50）定了幂等与重试策略再迁。
abstract final class ReplyHttp {
  /// 「刻意不带账号」的请求选项。评论这边已不再用它（匿名请求由 Rust 侧
  /// 按 ADR-011 自己构造上下文），保留是因为 `lib/http/dynamics.dart` 还在
  /// 用——它随 S5（#55）迁走时这里可以一并删掉。
  static final Options options = Options(
    headers: {...Constants.baseHeaders, 'cookie': ''},
    extra: {'account': const NoAccount()},
  );

  static Future<LoadingState<void>> hateReply({
    required int type,
    required int action,
    required int oid,
    required int rpid,
  }) async {
    final res = await Request().post(
      Api.hateReply,
      data: {
        'type': type,
        'oid': oid,
        'rpid': rpid,
        'action': action,
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

  // 评论点赞
  static Future<LoadingState<void>> likeReply({
    required int type,
    required int oid,
    required int rpid,
    required int action,
  }) async {
    final res = await Request().post(
      Api.likeReply,
      data: {
        'type': type,
        'oid': oid,
        'rpid': rpid,
        'action': action,
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

  static Future<LoadingState<void>> replyTop({
    required Object oid,
    required Object type,
    required Object rpid,
    required bool isUpTop,
  }) async {
    final res = await Request().post(
      Api.replyTop,
      data: {
        'oid': oid,
        'type': type,
        'rpid': rpid,
        'action': isUpTop ? 0 : 1,
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

  static Future<LoadingState<void>> report({
    required Object rpid,
    required Object oid,
    required int reasonType,
    bool banUid = true,
    String? reasonDesc,
  }) async {
    final res = await Request().post(
      Api.replyReport,
      data: {
        'add_blacklist': banUid,
        'csrf': Accounts.main.csrf,
        'gaia_source': 'main_h5',
        'oid': oid,
        'platform': 'android',
        'reason': reasonType,
        'rpid': rpid,
        'scene': 'main',
        'type': 1,
        'content': ?reasonDesc,
      },
      options: Options(contentType: Headers.formUrlEncodedContentType),
    );

    if (res.data['code'] == 0) {
      return const Success(null);
    } else {
      return Error(res.data['message']);
    }
  }

  static Future<LoadingState<void>> replySubjectModify({
    required int oid,
    required int type,
    required int action,
  }) async {
    final res = await Request().post(
      Api.replySubjectModify,
      data: {
        'oid': oid,
        'type': type,
        'action': action,
        'csrf': Accounts.main.csrf,
      },
      options: Options(contentType: Headers.formUrlEncodedContentType),
    );
    if (res.data['code'] == 0) {
      if (res.data['data']?['action_toast'] case final String toast) {
        SmartDialog.showToast(toast);
      }
      return const Success(null);
    } else {
      SmartDialog.showToast(res.data['message'].toString());
      return const Error(null);
    }
  }
}
