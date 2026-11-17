import 'package:kilikili/common/widgets/flutter/refresh_indicator.dart'
    show RefreshIndicatorState;
import 'package:kilikili/common/widgets/scroll_physics.dart' show ReloadMixin;
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/http/search.dart';
import 'package:kilikili/models/common/member/archive_order_type_app.dart';
import 'package:kilikili/models/common/member/archive_sort_type_app.dart';
import 'package:kilikili/models/common/member/contribute_type.dart';
import 'package:kilikili/models/common/video/source_type.dart';
import 'package:kilikili/models_new/space/space_archive/item.dart';
import 'package:kilikili/services/rust/rust_space.dart' as rust;
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/pages/member/controller.dart';
import 'package:kilikili/utils/extension/dimension_ext.dart';
import 'package:kilikili/utils/id_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:flutter/widgets.dart' show GlobalKey;
import 'package:get/get.dart';

class MemberVideoCtr
    extends CommonListController<rust.SpaceArchivePage, SpaceArchiveItem>
    with ReloadMixin {
  MemberVideoCtr({
    required this.heroTag,
    required this.type,
    required this.mid,
    required this.seasonId,
    required this.seriesId,
    this.username,
    this.title,
  }) : isVideo = type == .video;

  final String? heroTag;
  final ContributeType type;
  final bool isVideo;
  int? seasonId;
  int? seriesId;
  final int mid;
  late ArchiveOrderTypeApp order = .pubdate;
  late ArchiveSortTypeApp sort = .desc;
  int? count;
  int? next;
  rust.SpaceEpisodicButton? episodicButton;
  final String? username;
  final String? title;

  String? firstAid;
  String? lastAid;
  String? fromViewAid;
  final RxBool _isLocating = false.obs;
  bool get isLocating => _isLocating.value;
  void setIsLocating(bool value, {bool isOnlyInnerScroll = true}) {
    _isLocating.value = value;
    if (isOnlyInnerScroll) {
      onlyInnerScroll = value;
    }
  }

  set onlyInnerScroll(bool value) {
    final state = Get.find<MemberController>(tag: heroTag)
        .scrollKey
        .currentState;
    if (state != null && state.mounted) {
      state.onlyInnerScroll = value;
    }
  }

  bool isLoadPrevious = false;
  bool? hasPrev;

  @override
  GlobalKey<RefreshIndicatorState>? refreshKey;

  @override
  Future<void> onRefresh() {
    isLoadPrevious = false;
    firstAid = null;
    lastAid = null;
    next = null;
    isEnd = false;
    page = 0;
    return queryData();
  }

  @override
  void onInit() {
    super.onInit();
    if (isVideo) {
      fromViewAid = Get.parameters['from_view_aid'];
      if (fromViewAid?.isNotEmpty ?? false) {
        refreshKey = GlobalKey();
      }
    }
    page = 0;
    queryData();
  }

  @override
  bool customHandleResponse(
    bool isRefresh,
    Success<rust.SpaceArchivePage> response,
  ) {
    final data = response.response;
    episodicButton = data.episodicButton;
    next = data.next?.toInt();
    // Rust 的结构不可变，拼接落在这个本地列表上。
    final items = data.items.map(SpaceArchiveItem.fromRust).toList();
    if (page == 0 || isLoadPrevious) {
      hasPrev = data.hasPrev;
      if (isLoadPrevious && hasPrev != true) {
        onlyInnerScroll = false;
      }
    }
    if (page == 0 || !isLoadPrevious) {
      if ((isVideo ? data.hasNext == false : next == 0) || items.isEmpty) {
        isEnd = true;
      }
    }
    count = type == .season ? items.length : data.count?.toInt();
    if (page != 0) {
      if (loadingState.value case Success(:final response)) {
        if (isLoadPrevious) {
          items.addAll(response!);
        } else {
          items.insertAll(0, response!);
        }
      }
    }
    firstAid = items.firstOrNull?.param;
    lastAid = items.lastOrNull?.param;
    isLoadPrevious = false;
    loadingState.value = Success(items);
    return true;
  }

  @override
  Future<LoadingState<rust.SpaceArchivePage>> customGetData() =>
      rust.RustSpace.spaceArchive(
        kind: type.kind,
        mid: mid,
        aid: isVideo
            ? isLoadPrevious
                  ? firstAid
                  : lastAid
            : null,
        order: isVideo ? order.kind : null,
        sort: isVideo
            ? isLoadPrevious
                  ? rust.SpaceArchiveSort.asc
                  : null
            : sort.kind,
        page: type == .charging ? page : null,
        next: next,
        seasonId: seasonId,
        seriesId: seriesId,
        includeCursor: isLocating && page == 0,
      );

  void queryBySort() {
    if (isLoading) return;
    if (isVideo) {
      setIsLocating(false);
      order = order == .pubdate ? .click : .pubdate;
    } else {
      sort = sort == .desc ? .asc : .desc;
    }
    onReload();
  }

  Future<void> toViewPlayAll() async {
    final episodicButton = this.episodicButton!;
    if (episodicButton.text == '继续播放' &&
        episodicButton.uri?.isNotEmpty == true) {
      final params = Uri.parse(episodicButton.uri!).queryParameters;
      String? oid = params['oid'];
      if (oid != null) {
        final bvid = IdUtils.av2bv(int.parse(oid));
        final res = await SearchHttp.ab2cWithDimension(aid: oid, bvid: bvid);
        final cid = res?.cid;
        if (cid != null) {
          PageUtils.toVideoPage(
            aid: int.parse(oid),
            bvid: bvid,
            cid: cid,
            dimension: res!.dimension,
            // title: res.title,
            extraArguments: {
              'sourceType': SourceType.archive,
              'mediaId': seasonId ?? seriesId ?? mid,
              'oid': oid,
              'favTitle':
                  '$username: ${title ?? episodicButton.text ?? '播放全部'}',
              if (seriesId == null) 'count': ?count,
              if (seasonId != null || seriesId != null)
                'mediaType': params['page_type'],
              'desc': params['desc'] == '1',
              'sortField': params['sort_field'],
              'isContinuePlaying': true,
            },
          );
        }
      }
      return;
    }

    if (loadingState.value case Success(:final response)) {
      if (response == null || response.isEmpty) return;

      for (SpaceArchiveItem element in response) {
        if (element.cid == null) {
          continue;
        } else {
          bool desc = seasonId != null ? false : true;
          desc =
              (seasonId != null || seriesId != null) &&
                  (isVideo ? order == .click : sort == .asc)
              ? !desc
              : desc;
          bool isVertical = false;
          if (element.uri case final uri?) {
            isVertical = uri.isVerticalFromUri;
          }
          PageUtils.toVideoPage(
            bvid: element.bvid,
            cid: element.cid!,
            cover: element.cover,
            title: element.title,
            isVertical: isVertical,
            extraArguments: {
              'sourceType': SourceType.archive,
              'mediaId': seasonId ?? seriesId ?? mid,
              'oid': IdUtils.bv2av(element.bvid!),
              'favTitle':
                  '$username: ${title ?? episodicButton.text ?? '播放全部'}',
              if (seriesId == null) 'count': ?count,
              if (seasonId != null || seriesId != null)
                'mediaType': Uri.parse(
                  episodicButton.uri!,
                ).queryParameters['page_type'],
              'desc': desc,
              if (isVideo) 'sortField': order == .click ? 2 : 1,
            },
          );
          break;
        }
      }
    }
  }

  @override
  Future<void> onReload() {
    reload = true;
    setIsLocating(false);
    return super.onReload();
  }
}
