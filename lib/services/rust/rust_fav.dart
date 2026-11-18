import 'package:kilikili/common/constants.dart';
import 'package:kilikili/http/api.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/fav_order_type.dart';
import 'package:kilikili/rust/api/fav.dart' as rust_api;
import 'package:kilikili/rust/api/write.dart' show WriteResult;
import 'package:kilikili/services/rust/rust_request.dart';
import 'package:kilikili/services/rust/rust_user.dart' show SubList;
import 'package:kilikili/services/rust/rust_transport.dart';
import 'package:kilikili/services/rust_core.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:flutter/foundation.dart';

export 'package:kilikili/rust/api/fav.dart'
    show
        FavArticleItem,
        FavArticleList,
        FavDetail,
        FavFolder,
        FavFolderList,
        FavMedia,
        FavNote,
        FavPgcItem,
        FavPgcList,
        FavPugvItem,
        FavPugvList,
        FavSeasonDetail,
        FavSeasonMedia,
        FavTopicItem,
        FavTopicList,
        SpaceFavGroup;

/// 收藏夹那一族的取数（S8 / issue #58）：收藏夹、追番、课程、话题、专栏、
/// 笔记、合集，以及 22 条写操作。
///
/// 基线对应的 `FavHttp` 已随本次迁移整个删除，按 ADR-011 决策四与
/// ADR-013 决策五**不留降级开关**——写操作降级的代价是可能重复提交。
///
/// ## 账号
///
/// 这一族在 `ApiType.apiTypeSet` 里一条都没登记，与基线拦截器一样全走
/// `main`。仍然按 [RustRequest.accountFor] 查表而不是写死：登记表改了
/// 这里跟着变，硬编码则要靠人记得同步。
///
/// ## 「目标状态」是这一层的统一口径
///
/// 基线那批 `isFav` / `isLike` 参数传的是**当前**状态，正文再取反
/// （`isFav ? unfav : fav`）。Rust 侧统一收**目标**状态，折算只在本文件
/// 里做一次。传反了不报错，表现是按钮点一下没反应（ADR-013 补充五）。
///
/// ## 两条排序接口要额外的签名材料
///
/// `sortFav` / `sortFavFolder` 的 csrf **被 APP 签名覆盖**，因此 csrf 的
/// 注入点前移到了 Rust 的请求构造处，这里只多传一组
/// [rust_api.FavSignature]。理由见 `rust/crates/bili-api/src/fav.rs`
/// 的模块文档。
abstract final class RustFav {
  /// 与基线 `BaseOptions` 一致的超时（`lib/http/init.dart`）。
  static const int _timeoutMs = 10000;

  static const String _notReady = 'Rust Core 未就绪，该功能暂不可用';

  /// [write] 为 true 时才带 csrf（ADR-013 决策四）。
  ///
  /// 笔记列表那两条是 GET 却也要 csrf（基线把它放在查询串里），所以它们
  /// 也传 `write: true`——这个参数的含义是「要不要带 csrf」，不是「是不是
  /// 写操作」。
  static Future<rust_api.FavContext> _context(
    String path, {
    bool write = false,
    Map<String, String>? extraHeaders,
  }) async {
    final account = RustRequest.accountFor(path);
    final base = RustRequest.headersFor(account);
    return rust_api.FavContext(
      baseUrl: HttpString.apiBaseUrl,
      cookie: await RustRequest.cookieFor(account, path),
      csrf: write ? RustRequest.csrfFor(account) : null,
      headers: extraHeaders == null
          ? base
          : _withOverrides(base, extraHeaders),
      timeoutMs: BigInt.from(_timeoutMs),
      transport: RustTransport.current(),
    );
  }

  /// 逐键覆盖请求头。
  ///
  /// reqwest 会保留同名头的多个值；基线 Dio 的逐键覆盖只发最终值。不先
  /// 剔除同名项的话，个人空间收藏页会带着两个 `user-agent` 发出去。
  static List<(String, String)> _withOverrides(
    List<(String, String)> base,
    Map<String, String> extra,
  ) {
    final overrides = extra.keys.map((k) => k.toLowerCase()).toSet();
    return [
      for (final header in base)
        if (!overrides.contains(header.$1.toLowerCase())) header,
      ...extra.entries.map((e) => (e.key, e.value)),
    ];
  }

  /// 两条排序接口的 APP 签名材料。
  static rust_api.FavSignature get _signature => rust_api.FavSignature(
    appkey: Constants.appKey,
    appsec: Constants.appSec,
    ts: DateTime.now().millisecondsSinceEpoch ~/ 1000,
  );

  // ----------------------------------------------------------- 视频收藏夹

  /// 自己创建的收藏夹，分页。
  static Future<LoadingState<rust_api.FavFolderList>> userFavFolder({
    required int mid,
    required int pn,
    required int ps,
  }) => _read('收藏夹列表', () async {
    final r = await rust_api.fetchFavFolders(
      context: await _context(Api.userFavFolder),
      upMid: mid,
      pn: pn,
      ps: ps,
    );
    return (r.data, r.errorDetail, r.errorCode);
  });

  /// 自己创建的收藏夹，一次给全。
  static Future<LoadingState<rust_api.FavFolderList>> allFavFolders(int mid) =>
      _read('全部收藏夹', () async {
        final r = await rust_api.fetchAllFavFolders(
          context: await _context(Api.favFolder),
          upMid: mid,
        );
        return (r.data, r.errorDetail, r.errorCode);
      });

  /// 这个资源被收藏在哪几个夹子里。每一项会多带 `favState`。
  ///
  /// [rid] 收 `Object`：调用点的 `getFavRidType` 给的是 `Object`（稿件页
  /// 的 aid 来自路由参数，有的入口放进去的是字符串）。收紧成 `int` 只会
  /// 把问题推成运行期 `TypeError`（ADR-013 补充十二）。
  static Future<LoadingState<rust_api.FavFolderList>> videoInFolder({
    required int mid,
    required Object rid,
    int? type,
  }) async {
    final id = _asInt(rid);
    if (id == null) return Error('资源 id 不是数字: $rid');
    return _read('收藏状态', () async {
      final r = await rust_api.fetchVideoInFolder(
        context: await _context(Api.favFolder),
        upMid: mid,
        rid: id,
        kind: type,
      );
      return (r.data, r.errorDetail, r.errorCode);
    });
  }

  /// 单个收藏夹的信息。
  ///
  /// [mediaId] 收 `Object`：创建/编辑页的 id 来自路由参数
  /// （`Get.parameters['mediaId']`，是 `String`），而别的调用点给的是
  /// `int`。收紧成 `int` 不会在编译期报出来，只会在那一个入口上变成运行时
  /// `TypeError`（ADR-013 补充十二）。
  static Future<LoadingState<rust_api.FavFolder>> favFolderInfo(
    Object mediaId,
  ) async {
    final id = _asInt(mediaId);
    if (id == null) return Error('收藏夹 id 不是数字: $mediaId');
    return _read('收藏夹信息', () async {
      final r = await rust_api.fetchFavFolderInfo(
        context: await _context(Api.favFolderInfo),
        mediaId: id,
      );
      return (r.data, r.errorDetail, r.errorCode);
    });
  }

  /// 收藏夹详情的一页。[pageDesc] 为真时整页倒序。
  static Future<LoadingState<rust_api.FavDetail>> favDetail({
    required int mediaId,
    required int pn,
    required int ps,
    String keyword = '',
    FavOrderType order = FavOrderType.mtime,
    int type = 0,
    bool pageDesc = false,
  }) => _read('收藏夹详情', () async {
    final r = await rust_api.fetchFavDetail(
      context: await _context(Api.favResourceList),
      mediaId: mediaId,
      pn: pn,
      ps: ps,
      keyword: keyword,
      order: order.rust,
      kind: type,
      pageDesc: pageDesc,
    );
    return (r.data, r.errorDetail, r.errorCode);
  });

  /// 合集详情的一页。
  static Future<LoadingState<rust_api.FavSeasonDetail>> favSeasonList({
    required int seasonId,
    required int pn,
    required int ps,
    bool pageDesc = false,
  }) => _read('合集详情', () async {
    final r = await rust_api.fetchFavSeasonDetail(
      context: await _context(Api.favSeasonList),
      seasonId: seasonId,
      pn: pn,
      ps: ps,
      pageDesc: pageDesc,
    );
    return (r.data, r.errorDetail, r.errorCode);
  });

  /// 个人空间「收藏」页的翻页：把收藏夹列表读成 `SubItem`。
  ///
  /// 与 [userFavFolder] 是同一条接口，只是解成另一个形状——那一页两组
  /// （收藏夹 / 订阅的合集）共用一套卡片，而卡片要的是 `SubItem`。订阅那
  /// 一组的翻页走 `RustUser.userSubFolder`，两边于是同一个结果类型。
  static Future<LoadingState<SubList>> spaceFavFolderPage({
    required int mid,
    required int pn,
    required int ps,
  }) => _read('空间收藏夹翻页', () async {
    final r = await rust_api.fetchFavFoldersAsSubs(
      context: await _context(Api.userFavFolder),
      upMid: mid,
      pn: pn,
      ps: ps,
    );
    return (r.data, r.errorDetail, r.errorCode);
  });

  /// 个人空间的收藏页。
  ///
  /// 这条接口伪装成移动端客户端：参数那一组在 Rust 侧，两条请求头的真相
  /// 源在宿主，由这里带上。少带一条会换回另一套分组，不会报错。
  static Future<LoadingState<List<rust_api.SpaceFavGroup>>> spaceFav(
    int mid,
  ) => _read('空间收藏', () async {
    final r = await rust_api.fetchSpaceFav(
      context: await _context(
        Api.spaceFav,
        extraHeaders: const {
          'bili-http-engine': 'cronet',
          'user-agent': Constants.userAgentApp,
        },
      ),
      upMid: mid,
    );
    return (r.data, r.errorDetail, r.errorCode);
  });

  // --------------------------------------------- 追番 / 课程 / 话题 / 专栏

  /// 追番/追剧列表的一页。[type] 为 1 番剧、2 影视。
  static Future<LoadingState<rust_api.FavPgcList>> favPgc({
    required int type,
    required int pn,
    int? followStatus,
    int? mid,
  }) => _read('追番列表', () async {
    final r = await rust_api.fetchFavPgc(
      context: await _context(Api.favPgc),
      vmid: mid ?? Accounts.main.mid,
      kind: type,
      followStatus: followStatus,
      pn: pn,
    );
    return (r.data, r.errorDetail, r.errorCode);
  });

  /// 收藏的课程一页。
  static Future<LoadingState<rust_api.FavPugvList>> favPugv({
    required int mid,
    required int page,
  }) => _read('课程收藏', () async {
    final r = await rust_api.fetchFavPugv(
      context: await _context(Api.favPugv),
      mid: mid,
      pn: page,
    );
    return (r.data, r.errorDetail, r.errorCode);
  });

  /// 收藏的话题一页。
  static Future<LoadingState<rust_api.FavTopicList>> favTopic({
    required int page,
  }) => _read('话题收藏', () async {
    final r = await rust_api.fetchFavTopics(
      context: await _context(Api.favTopicList),
      page: page,
    );
    return (r.data, r.errorDetail, r.errorCode);
  });

  /// 收藏的专栏一页。
  static Future<LoadingState<rust_api.FavArticleList>> favArticle({
    required int page,
  }) => _read('专栏收藏', () async {
    final r = await rust_api.fetchFavArticles(
      context: await _context(Api.favArticle),
      page: page,
    );
    return (r.data, r.errorDetail, r.errorCode);
  });

  /// 笔记列表的一页。[published] 为真时取「我发布的笔记」。
  static Future<LoadingState<List<rust_api.FavNote>>> noteList({
    required int page,
    required bool published,
  }) => _read('笔记列表', () async {
    final r = await rust_api.fetchFavNotes(
      context: await _context(
        published ? Api.userNoteList : Api.noteList,
        write: true,
      ),
      page: page,
      published: published,
    );
    return (r.data, r.errorDetail, r.errorCode);
  });

  // --------------------------------------------------------------- 写操作

  /// 收藏 / 取消收藏别人的收藏夹。[fav] 是目标状态。
  static Future<LoadingState<void>> favFavFolder({
    required int mediaId,
    required bool fav,
  }) => _write('收藏收藏夹', () async {
    return rust_api.favFolder(
      context: await _context(Api.favFavFolder, write: true),
      mediaId: mediaId,
      fav: fav,
    );
  });

  /// 收藏 / 取消收藏一个合集。[fav] 是目标状态。
  static Future<LoadingState<void>> favSeason({
    required int seasonId,
    required bool fav,
  }) => _write('收藏合集', () async {
    return rust_api.favSeason(
      context: await _context(Api.favSeason, write: true),
      seasonId: seasonId,
      fav: fav,
    );
  });

  /// 取消订阅。[type] 为 11 时是收藏夹，否则是合集——两条不同的接口。
  static Future<LoadingState<void>> cancelSub({
    required int id,
    required int type,
  }) => type == 11
      ? favFavFolder(mediaId: id, fav: false)
      : favSeason(seasonId: id, fav: false);

  /// 收藏 / 取消收藏一门课程。[fav] 是目标状态。
  static Future<LoadingState<void>> favPugvWrite({
    required int seasonId,
    required bool fav,
  }) => _write('收藏课程', () async {
    return rust_api.favPugv(
      context: await _context(
        fav ? Api.addFavPugv : Api.delFavPugv,
        write: true,
      ),
      seasonId: seasonId,
      fav: fav,
    );
  });

  /// 收藏 / 取消收藏一个话题。[fav] 是目标状态。
  ///
  /// [topicId] 收 `Object`：话题页的 id 来自路由参数，是 `String`
  /// （`Get.parameters['id']`），而话题收藏页给的是 `int`。收紧成 `int`
  /// 只会把问题从编译期推到某一个入口的运行期（ADR-013 补充十二）。
  static Future<LoadingState<void>> favTopicWrite({
    required Object topicId,
    required bool fav,
  }) async {
    final id = _asInt(topicId);
    if (id == null) return Error('话题 id 不是数字: $topicId');
    return _write('收藏话题', () async {
      return rust_api.favTopic(
        context: await _context(
          fav ? Api.addFavTopic : Api.delFavTopic,
          write: true,
        ),
        topicId: id,
        fav: fav,
      );
    });
  }

  /// 点赞 / 取消点赞一个话题。[like] 是目标状态。
  static Future<LoadingState<void>> likeTopic({
    required Object topicId,
    required bool like,
  }) async {
    final id = _asInt(topicId);
    if (id == null) return Error('话题 id 不是数字: $topicId');
    return _write('点赞话题', () async {
      return rust_api.likeTopic(
        context: await _context(Api.likeTopic, write: true),
        topicId: id,
        upMid: Accounts.main.mid,
        like: like,
      );
    });
  }

  /// 收藏 / 取消收藏一篇专栏（旧 article 接口）。[fav] 是目标状态。
  static Future<LoadingState<void>> favArticleWrite({
    required int id,
    required bool fav,
  }) => _write('收藏专栏', () async {
    return rust_api.favArticle(
      context: await _context(
        fav ? Api.addFavArticle : Api.delFavArticle,
        write: true,
      ),
      id: id,
      fav: fav,
    );
  });

  /// 图文动态（opus）的收藏 / 取消收藏。[action]：3 收藏，4 取消。
  static Future<LoadingState<void>> communityAction({
    required String opusId,
    required int action,
  }) => _write('收藏图文', () async {
    return rust_api.communityAction(
      context: await _context(Api.communityAction, write: true),
      opusId: opusId,
      action: action,
    );
  });

  /// 删除笔记。[ids] 是逗号分隔的一串。
  static Future<LoadingState<void>> delNote({
    required String ids,
    required bool published,
  }) => _write('删除笔记', () async {
    return rust_api.delNote(
      context: await _context(
        published ? Api.delPublishNote : Api.delNote,
        write: true,
      ),
      ids: ids,
      published: published,
    );
  });

  /// 收藏夹排序。[sort] 是逗号分隔的 id 串，顺序即目标顺序。
  static Future<LoadingState<void>> sortFavFolder({required String sort}) =>
      _write('收藏夹排序', () async {
        return rust_api.sortFavFolder(
          context: await _context(Api.sortFavFolder, write: true),
          signature: _signature,
          sort: sort,
        );
      });

  /// 收藏夹内容排序。
  static Future<LoadingState<void>> sortFav({
    required int mediaId,
    required String sort,
  }) => _write('内容排序', () async {
    return rust_api.sortFav(
      context: await _context(Api.sortFav, write: true),
      signature: _signature,
      mediaId: mediaId,
      sort: sort,
    );
  });

  /// 清除收藏夹里的失效内容。
  static Future<LoadingState<void>> cleanFav({required int mediaId}) =>
      _write('清除失效', () async {
        return rust_api.cleanFav(
          context: await _context(Api.cleanFav, write: true),
          mediaId: mediaId,
        );
      });

  /// 删除收藏夹。[mediaIds] 是逗号分隔的一串。
  static Future<LoadingState<void>> deleteFolder({
    required String mediaIds,
  }) => _write('删除收藏夹', () async {
    return rust_api.deleteFolder(
      context: await _context(Api.deleteFolder, write: true),
      mediaIds: mediaIds,
    );
  });

  /// 新建 / 编辑收藏夹，返回操作后的那一个。
  ///
  /// 两条是**不同的接口、不同的幂等级别**：新建定级 `Unsafe`（重发会多建
  /// 一个同名夹子，不自动重试），编辑是 `Idempotent`。所以 Rust 侧分成两个
  /// 导出，这里按 [mediaId] 在不在挑——与基线 `addOrEditFolder` 的
  /// `isAdd` 一致。
  /// [mediaId] 收 `Object?`，理由同 [favFolderInfo]。
  static Future<LoadingState<rust_api.FavFolder>> addOrEditFolder({
    required bool isAdd,
    Object? mediaId,
    required String title,
    required int privacy,
    required String cover,
    required String intro,
  }) async {
    final id = mediaId == null ? null : _asInt(mediaId);
    if (!isAdd && id == null) {
      return Error('收藏夹 id 不是数字: $mediaId');
    }
    return _read('编辑收藏夹', () async {
      final context = await _context(
        isAdd ? Api.addFolder : Api.editFolder,
        write: true,
      );
      final r = isAdd
          ? await rust_api.addFolder(
              context: context,
              title: title,
              intro: intro,
              privacy: privacy,
              cover: cover,
            )
          : await rust_api.editFolder(
              context: context,
              mediaId: id!,
              title: title,
              intro: intro,
              privacy: privacy,
              cover: cover,
            );
      return (r.data, r.errorDetail, r.errorCode);
    });
  }

  /// 批量收藏 / 取消收藏某个资源。
  ///
  /// [resources] 是 `id:type` 形式；[addIds] / [delIds] 是逗号分隔的收藏夹
  /// id 串，空串表示这一侧不动。
  static Future<LoadingState<void>> favVideo({
    required String resources,
    String? addIds,
    String? delIds,
  }) => _write('收藏视频', () async {
    return rust_api.favVideo(
      context: await _context(Api.favVideo, write: true),
      resources: resources,
      addIds: addIds ?? '',
      delIds: delIds ?? '',
    );
  });

  /// 把一个资源从所有收藏夹里移出。[rid] 收 `Object`，同 [videoInFolder]。
  static Future<LoadingState<void>> unfavAll({
    required Object rid,
    required int type,
  }) async {
    final id = _asInt(rid);
    if (id == null) return Error('资源 id 不是数字: $rid');
    return _write('取消收藏', () async {
      return rust_api.unfavAll(
        context: await _context(Api.unfavAll, write: true),
        rid: id,
        kind: type,
      );
    });
  }

  /// 在收藏夹之间复制 / 移动资源。
  static Future<LoadingState<void>> copyOrMoveFav({
    required bool isCopy,
    required bool isFav,
    int? srcMediaId,
    required int tarMediaId,
    int? mid,
    required String resources,
  }) => _write('复制/移动', () async {
    final mode = isCopy
        ? rust_api.FavMoveMode.copy
        : rust_api.FavMoveMode.move;
    final target = isFav
        ? rust_api.FavMoveTarget.folder
        : rust_api.FavMoveTarget.toView;
    return rust_api.copyOrMoveFav(
      context: await _context(
        _copyOrMoveEndpoint(isCopy: isCopy, isFav: isFav),
        write: true,
      ),
      mode: mode,
      target: target,
      srcMediaId: srcMediaId,
      tarMediaId: tarMediaId,
      mid: mid,
      resources: resources,
    );
  });

  /// 四条路径对应的 `Api.*` 常量，用来查账号与 Cookie。
  ///
  /// 路径本身在 Rust 侧选；这里只是把同一个四选一再写一遍给账号查表用。
  /// 两处必须同步——查错了不会报错，只会用另一个账号发请求。
  static String _copyOrMoveEndpoint({
    required bool isCopy,
    required bool isFav,
  }) => isFav
      ? (isCopy ? Api.copyFav : Api.moveFav)
      : (isCopy ? Api.copyToview : Api.moveToview);

  // ----------------------------------------------------------------- 内部

  /// 把路由带来的标识符折算成数字，折不出就不发请求。
  ///
  /// 跨 FFI 的那一侧仍然是 `i64`——宽容只在宿主这一层，协议那一层不含糊
  /// （ADR-013 补充十二）。
  static int? _asInt(Object value) => switch (value) {
    final int v => v,
    final String v => int.tryParse(v),
    _ => null,
  };

  static Future<LoadingState<T>> _read<T>(
    String what,
    Future<(T?, String?, int?)> Function() run,
  ) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final (T?, String?, int?) r;
    try {
      r = await run();
    } catch (e, st) {
      return _crashed(what, e, st);
    }
    // 折算放在 try 之外：T 是泛型，可能本身就是 Future，在 try 里 return
    // 一个 T 会被 unawaited_return_in_try_block 盯上。
    final data = r.$1;
    return data == null ? Error(r.$2 ?? '请求失败', code: r.$3) : Success(data);
  }

  static Future<LoadingState<void>> _write(
    String what,
    Future<WriteResult> Function() run,
  ) async {
    if (!RustCore.isReady) return const Error(_notReady);
    final WriteResult result;
    try {
      result = await run();
    } catch (e, st) {
      return _crashed(what, e, st);
    }
    return result.errorKind == null
        ? const Success(null)
        : Error(result.errorDetail ?? '请求失败', code: result.errorCode);
  }

  /// 桥接层自身出错（未就绪、参数编码失败等）。不抛给 UI。
  static LoadingState<T> _crashed<T>(String what, Object e, StackTrace st) {
    if (kDebugMode) debugPrint('[rust] $what 出错: $e\n$st');
    return Error('$e');
  }
}

/// 界面上的排序枚举 → 协议取值。
///
/// 枚举本身留在宿主，因为它承载的是**界面标签**（「最近收藏」…），那是
/// i18n 文案（AGENTS.md §2）。协议取值在 Rust 的 `FavOrder` 上，这里只做
/// 一次映射——不要在调用点写 `order.name`，那等于把协议取值又散回宿主。
extension FavOrderTypeRust on FavOrderType {
  rust_api.FavOrder get rust => switch (this) {
    FavOrderType.mtime => rust_api.FavOrder.mtime,
    FavOrderType.view => rust_api.FavOrder.view,
    FavOrderType.pubtime => rust_api.FavOrder.pubtime,
  };
}
