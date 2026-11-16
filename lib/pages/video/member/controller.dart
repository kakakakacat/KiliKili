import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/http/member.dart';
import 'package:kilikili/models/common/member/archive_order_type_app.dart';
import 'package:kilikili/models_new/space/space_archive/data.dart';
import 'package:kilikili/services/rust/rust_space.dart';
import 'package:kilikili/models_new/space/space_archive/item.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/extension/scroll_controller_ext.dart';
import 'package:get/get.dart';

class HorizontalMemberPageController
    extends CommonListController<SpaceArchiveData, SpaceArchiveItem> {
  HorizontalMemberPageController({this.mid, required this.currAid});

  dynamic mid;

  final Rx<LoadingState<MemberInfo>> userState =
      LoadingState<MemberInfo>.loading().obs;

  /// 关注态单独拿出来。
  ///
  /// 基线是就地改 `userState.value.data.isFollowed`；`MemberInfo` 来自
  /// Rust，字段是 final 的。关注与否本来也**不是这条接口的事实**，它会被
  /// 另一条写接口改掉，按 AGENTS.md §2 属于界面状态，归控制器。
  final RxnBool isFollowed = RxnBool();

  /// 关注数 / 粉丝数 / 获赞数，各自来自不同接口，按需填。
  final RxnInt following = RxnInt();
  final RxnInt follower = RxnInt();
  final RxnInt likes = RxnInt();

  @override
  void onInit() {
    super.onInit();
    getUserInfo();
    queryData();
  }

  Future<void> getUserInfo() async {
    final res = await RustSpace.memberInfo(mid: mid);
    userState.value = res;
    if (res case Success(:final response)) {
      isFollowed.value = response.isFollowed;
      getMemberStat();
      getMemberView();
    }
  }

  Future<void> getMemberStat() async {
    final res = await RustSpace.memberRelationStat(mid: mid);
    if (res case Success(:final response)) {
      following.value = response.following;
      follower.value = response.follower;
    }
  }

  Future<void> getMemberView() async {
    if (!Accounts.main.isLogin) {
      return;
    }
    final res = await RustSpace.memberUpStat(mid: mid);
    if (res case Success(:final response)) {
      likes.value = response.likes;
    }
  }

  @override
  bool customHandleResponse(bool isRefresh, Success response) {
    SpaceArchiveData data = response.response;
    count = data.count;
    if (isRefresh) {
      if (isLoadPrevious) {
        hasPrev = data.hasPrev ?? false;
      } else {
        hasNext = data.hasNext ?? false;
      }
    }
    if (isLoadPrevious) {
      if (loadingState.value case Success(:final response)) {
        (data.item ??= <SpaceArchiveItem>[]).addAll(response!);
      }
    } else if (!isRefresh) {
      if (loadingState.value case Success(:final response)) {
        (data.item ??= <SpaceArchiveItem>[]).insertAll(0, response!);
      }
    }
    firstAid = data.item?.firstOrNull?.param;
    lastAid = data.item?.lastOrNull?.param;
    loadingState.value = Success(data.item);
    isLoadPrevious = false;
    page++;
    return true;
  }

  String? currAid;
  String? firstAid;
  String? lastAid;
  ArchiveOrderTypeApp order = .pubdate;
  int? count;
  bool isLoadPrevious = false;
  bool hasPrev = true;
  bool hasNext = true;

  @override
  Future<LoadingState<SpaceArchiveData>> customGetData() =>
      MemberHttp.spaceArchive(
        type: .video,
        mid: mid,
        aid: page == 1
            ? currAid
            : isLoadPrevious
            ? firstAid
            : lastAid,
        order: order,
        sort: page != 1 && isLoadPrevious ? .asc : null,
        pn: null,
        next: null,
        seasonId: null,
        seriesId: null,
        includeCursor: page == 1 ? true : null,
      );

  @override
  Future<void> onRefresh() {
    assert(hasPrev);
    isLoadPrevious = true;
    return queryData();
  }

  @override
  Future<void> onReload() {
    firstAid = null;
    lastAid = null;
    hasNext = true;
    hasPrev = true;
    isEnd = false;
    page = 1;
    scrollController.jumpToTop();
    return super.onReload();
  }

  void queryBySort() {
    if (isLoading) return;
    order = order == .pubdate ? .click : .pubdate;
    onReload();
  }
}
