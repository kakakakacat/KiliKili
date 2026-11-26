#!/usr/bin/env python3
"""在 build 内生成完整产品的独立 profile 包，不改宿主 SDK、原应用或账号。"""
import argparse
import io
from pathlib import Path
import shutil
import subprocess
import tarfile

root = Path(__file__).resolve().parents[2]
p = argparse.ArgumentParser()
p.add_argument('--revision', default='HEAD')
p.add_argument('--name', default='candidate')
p.add_argument('--reuse-build', action='store_true')
a = p.parse_args()
app = root / 'build' / f'navigation-profile-{a.name}'
app.mkdir(parents=True, exist_ok=True)
archive = subprocess.check_output(['git', 'archive', a.revision], cwd=root)
with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
    members = tar.getmembers()
    if a.reuse_build:
        members = [m for m in members if m.name.startswith('lib/') or m.name in ['android/app/build.gradle.kts', 'android/app/src/main/kotlin/io/github/kakacat/kilikili/MainActivity.kt']]
    tar.extractall(app, members=members, filter='data')


def replace(relative, old, new):
    path = app / relative
    text = path.read_text()
    if old not in text:
        raise RuntimeError(f'{relative}: 未找到待插桩位置，停止以免生成失真的测量包')
    path.write_text(text.replace(old, new))


helper = "import 'package:kilikili/navigation_profile.dart';\n"
for file in ['lib/pages/main/controller.dart', 'lib/pages/main/view.dart',
             'lib/pages/common/common_list_controller.dart', 'lib/pages/common/common_data_controller.dart',
             'lib/pages/mine/controller.dart']:
    path = app / file
    path.write_text(helper + path.read_text())
replace('lib/pages/main/controller.dart', '  void setIndex(int value) {',
        '  void setIndex(int value) {\n    NavigationProfile.select(value);')
replace('lib/pages/main/controller.dart', '      selectedIndex.value = value;',
        "      selectedIndex.value = value;\n      NavigationProfile.event('selected');")
replace('lib/pages/main/view.dart',
        '_mainController.navigationBars.map((i) => i.page).toList()',
        '_mainController.navigationBars.indexed.map((e) => NavigationProfilePage(index: e.$1, child: e.$2.page)).toList()')
for file in ['lib/pages/common/common_list_controller.dart', 'lib/pages/common/common_data_controller.dart']:
    replace(file, '    final LoadingState<R> res = await customGetData();',
            "    final LoadingState<R> res = await customGetData();\n    NavigationProfile.event('request_result', {'controller': runtimeType.toString(), 'state': res.runtimeType.toString()});")
for method in ['queryUserInfo', 'queryUserStatOwner']:
    replace('lib/pages/mine/controller.dart', f'  Future<void> {method}() async {{',
            f"  Future<void> {method}() async {{\n    NavigationProfile.minePending.add('{method}');")
for method, request in [('queryUserInfo', 'userInfo'), ('queryUserStatOwner', 'userStatOwner')]:
    replace('lib/pages/mine/controller.dart', f'    final res = await RustUser.{request}();',
            f"    final res = await RustUser.{request}();\n    NavigationProfile.minePending.remove('{method}');\n    NavigationProfile.event('request_result', {{'controller': 'MineController.{method}', 'state': res.runtimeType.toString()}});")
replace('lib/main.dart', 'void main() async {', 'Future<void> main() async {')
replace('lib/main.dart', '    await GStorage.init();', '''    await GStorage.init();
    await GStorage.setting.put(SettingBoxKey.autoUpdate, false);
    await GStorage.setting.put(SettingBoxKey.mainTabBarView, false);
    await GStorage.setting.put(SettingBoxKey.floatingNavBar, true);
    await GStorage.setting.put(SettingBoxKey.defaultHomePage, 0);''')
# profile 使用单独包名和本机 debug 身份，不读取产品签名材料。
replace('android/app/build.gradle.kts', '    buildTypes {', '''    buildTypes {
        getByName("profile") {
            applicationIdSuffix = ".navprofile"
            signingConfig = signingConfigs.getByName("debug")
        }''')
replace('android/app/src/main/kotlin/io/github/kakacat/kilikili/MainActivity.kt',
        'class MainActivity : AudioServiceActivity() {', '''class MainActivity : AudioServiceActivity() {
    override fun configureFlutterEngine(engine: io.flutter.embedding.engine.FlutterEngine) {
        super.configureFlutterEngine(engine)
        io.flutter.plugin.common.MethodChannel(engine.dartExecutor.binaryMessenger, "navigation_profile")
            .setMethodCallHandler { call, result ->
                if (call.method == "target") result.success(intent.getStringExtra("nav_target") ?: "dynamics")
                else result.notImplemented()
            }
    }''')
shutil.copy(root / 'android/local.properties', app / 'android/local.properties')
shutil.copy(root / 'tool/navigation_profile/probe.dart', app / 'lib/navigation_profile.dart')
shutil.copy(root / 'tool/navigation_profile/main.dart', app / 'lib/navigation_profile_main.dart')
flutter_sdk = next(line.split('=', 1)[1].strip() for line in (root / 'android/local.properties').read_text().splitlines() if line.startswith('flutter.sdk='))
subprocess.run([shutil.which('flutter') or str(Path(flutter_sdk) / 'bin/flutter'), 'pub', 'get', '--offline'], cwd=app, check=True, stdout=subprocess.DEVNULL)
print(app)
