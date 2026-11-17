import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/member/archive_order_type_app.dart';
import 'package:kilikili/models/common/member/contribute_type.dart';
import 'package:kilikili/models_new/space/space_archive/item.dart';
import 'package:kilikili/pages/common/common_list_controller.dart';
import 'package:kilikili/services/rust/rust_space.dart' as rust;
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/extension/scroll_controller_ext.dart';
import 'package:get/get.dart';

class HorizontalMemberPageController
    extends CommonListController<rust.SpaceArchivePage, SpaceArchiveItem> {
  HorizontalMemberPageController({this.mid, required this.currAid});

  dynamic mid;

  final Rx<LoadingState<rust.MemberInfo>> userState =
      LoadingState<rust.MemberInfo>.loading().obs;

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
    final res = await rust.RustSpace.memberInfo(mid: mid);
    userState.value = res;
    if (res case Success(:final response)) {
      isFollowed.value = response.isFollowed;
      getMemberStat();
      getMemberView();
    }
  }

  Future<void> getMemberStat() async {
    final res = await rust.RustSpace.memberRelationStat(mid: mid);
    if (res case Success(:final response)) {
      following.value = response.following;
      follower.value = response.follower;
    }
  }

  Future<void> getMemberView() async {
    if (!Accounts.main.isLogin) {
      return;
    }
    final res = await rust.RustSpace.memberUpStat(mid: mid);
    if (res case Success(:final response)) {
      likes.value = response.likes;
    }
  }

  @override
  bool customHandleResponse(bool isRefresh, Success response) {
    final rust.SpaceArchivePage data = response.response;
    count = data.count?.toInt();
    // Rust 的结构不可变，拼接落在这个本地列表上。
    final items = data.items.map(SpaceArchiveItem.fromRust).toList();
    if (isRefresh) {
      if (isLoadPrevious) {
        hasPrev = data.hasPrev ?? false;
      } else {
        hasNext = data.hasNext ?? false;
      }
    }
    if (isLoadPrevious) {
      if (loadingState.value case Success(:final response)) {
        items.addAll(response!);
      }
    } else if (!isRefresh) {
      if (loadingState.value case Success(:final response)) {
        items.insertAll(0, response!);
      }
    }
    firstAid = items.firstOrNull?.param;
    lastAid = items.lastOrNull?.param;
    loadingState.value = Success(items);
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
  Future<LoadingState<rust.SpaceArchivePage>> customGetData() =>
      rust.RustSpace.spaceArchive(
        kind: ContributeType.video.kind,
        mid: mid,
        aid: page == 1
            ? currAid
            : isLoadPrevious
            ? firstAid
            : lastAid,
        order: order.kind,
        sort: page != 1 && isLoadPrevious ? rust.SpaceArchiveSort.asc : null,
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
