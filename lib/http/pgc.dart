import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/init.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:dio/dio.dart';

/// 本文件里的**只读接口已经迁到 Rust**（S10 第一批 / issue #60）：索引结果、
/// 索引筛选条件、番剧索引、放送时间表、短评/长评列表、追番状态现在在
/// `lib/services/rust/rust_pgc.dart`；两种排行榜原本住在
/// `lib/http/video.dart`，也一并迁了过去。
///
/// 剩下这五个是**写操作**，刻意留在原地：它们归 ADR-013（issue #50）——
/// 写操作的幂等、CSRF 与重试策略没定之前不迁。短评的点赞与点踩是可重放的
/// （重复提交只是状态不变），但发表、修改、删除不是，三者需要同一套结论
/// 才能一起动，拆开迁会让这个文件在两种约定之间悬着。
///
/// 番剧详情（`pgcInfo` / `pugvInfo`）仍在 `lib/http/search.dart`：它的
/// `PgcInfoModel` 深入播放页，留到 S10 第二批。
abstract final class PgcHttp {
  static Future<LoadingState<void>> pgcReviewLike({
    required Object mediaId,
    required Object reviewId,
  }) async {
    final res = await Request().post(
      Api.pgcReviewLike,
      data: {
        'media_id': mediaId,
        'review_type': 2,
        'review_id': reviewId,
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

  static Future<LoadingState<void>> pgcReviewDislike({
    required Object mediaId,
    required Object reviewId,
  }) async {
    final res = await Request().post(
      Api.pgcReviewDislike,
      data: {
        'media_id': mediaId,
        'review_type': 2,
        'review_id': reviewId,
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

  static Future<LoadingState<void>> pgcReviewPost({
    required Object mediaId,
    required int score,
    required String content,
    bool shareFeed = false,
  }) async {
    final res = await Request().post(
      Api.pgcReviewPost,
      data: {
        'media_id': mediaId,
        'score': score,
        'content': content,
        if (shareFeed) 'share_feed': 1,
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

  static Future<LoadingState<void>> pgcReviewMod({
    required Object mediaId,
    required int score,
    required String content,
    required reviewId,
  }) async {
    final res = await Request().post(
      Api.pgcReviewMod,
      data: {
        'media_id': mediaId,
        'score': score,
        'content': content,
        'review_id': reviewId,
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

  static Future<LoadingState<void>> pgcReviewDel({
    required Object mediaId,
    required Object reviewId,
  }) async {
    final res = await Request().post(
      Api.pgcReviewDel,
      data: {
        'media_id': mediaId,
        'review_id': reviewId,
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
