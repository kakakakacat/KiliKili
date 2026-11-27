import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/member/controller.dart';
import 'package:kilikili/services/rust/rust_space.dart';
import 'package:kilikili/utils/accounts/account.dart';

class _FailingMemberController extends MemberController {
  _FailingMemberController() : super(mid: 1);

  @override
  final account = LoginAccount(
    BiliCookieJar.fromJson({'DedeUserID': '99', 'buvid3': 'test-buvid'}),
    null,
    null,
    {},
    'test-buvid',
  );

  @override
  Future<LoadingState<SpacePage>> customGetData() async =>
      const Error('code -400: 请求错误');
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('资料失败不把错误写入昵称，重试保留可用标签', (tester) async {
    final controller = _FailingMemberController();
    await controller.queryData();
    expect(controller.username, isNot('code -400: 请求错误'));
    expect(controller.profileError.value, 'code -400: 请求错误');
    expect(controller.tab2!.map((tab) => tab.param), [
      'dynamic',
      'contribute',
      'favorite',
      'bangumi',
    ]);
    final tabs = controller.tabController!;
    tabs.index = 1;
    await controller.onReload();
    expect(controller.tabController, same(tabs));
    expect(controller.tabController!.index, 1);
    controller.onClose();
  });

  testWidgets('已有昵称不会被请求错误覆盖', (tester) async {
    final controller = _FailingMemberController()..username = '已有昵称';
    await controller.queryData();
    expect(controller.username, '已有昵称');
    controller.onClose();
  });

  testWidgets('资料刷新失败保留成功结果与当前标签', (tester) async {
    final controller = _FailingMemberController()..username = '已有昵称';
    await controller.queryData();
    const page = SpacePage(
      hasItem: false,
      hasSeasonOrSeries: false,
      tab2: [],
      reservations: [],
    );
    const loaded = Success<SpacePage>(page);
    controller.loadingState.value = loaded;
    final tabs = controller.tabController!;
    tabs.index = 2;
    await controller.onReload();
    expect(controller.loadingState.value, same(loaded));
    expect(controller.username, '已有昵称');
    expect(controller.tabController, same(tabs));
    expect(tabs.index, 2);
    expect(controller.profileError.value, 'code -400: 请求错误');
    controller.onClose();
  });

  testWidgets('没有标签的已加载资料在失败后仍保留，并提供兜底标签', (tester) async {
    final controller = _FailingMemberController();
    const loaded = Success<SpacePage>(
      SpacePage(
        hasItem: false,
        hasSeasonOrSeries: false,
        tab2: [],
        reservations: [],
      ),
    );
    controller.loadingState.value = loaded;
    var updates = 0;
    final subscription = controller.loadingState.listen((_) => updates++);
    await controller.onReload();
    expect(controller.loadingState.value, same(loaded));
    expect(controller.tabController, isNotNull);
    expect(updates, 1, reason: '兜底标签创建后需要通知外层页面重建');
    await subscription.cancel();
    controller.onClose();
  });

  testWidgets('重试成功清除资料错误并显示真实昵称', (tester) async {
    final controller = _FailingMemberController();
    await controller.queryData();
    const loaded = Success<SpacePage>(
      SpacePage(
        hasItem: false,
        hasSeasonOrSeries: false,
        tab2: [],
        reservations: [],
        card: SpaceCard(name: '真实昵称', tags: []),
      ),
    );
    controller.customHandleResponse(true, loaded);
    expect(controller.profileError.value, isNull);
    expect(controller.username, '真实昵称');
    expect(controller.loadingState.value, same(loaded));
    controller.onClose();
  });
}
