#!/usr/bin/env python3
"""生成独立验收宿主，依赖与源代码均复用当前 checkout，不修改产品工程。"""
import json
import os
from pathlib import Path
import shutil
import subprocess
from urllib.parse import unquote, urljoin, urlparse

root = Path(__file__).resolve().parents[2]
app = root / 'build/player-port-gui'
subprocess.run(['flutter', 'create', '--offline', '--platforms=macos,android,ios',
                '--project-name', 'kilikili', '--org', 'io.github.kilikili.playerportprobe',
                '--empty', str(app)], check=True, stdout=subprocess.DEVNULL)
config = root / '.dart_tool/package_config.json'
packages = json.loads(config.read_text())['packages']
paths = {p['name']: unquote(urlparse(urljoin(config.as_uri(), p['rootUri'])).path)
         for p in packages}
primary = ['flutter_rust_bridge', 'ffi', 'media_kit', 'media_kit_video',
           'media_kit_libs_video', 'canvas_danmaku']
text = 'name: kilikili\nversion: 0.0.1+1\npublish_to: none\nenvironment:\n  sdk: ">=3.13.0 <4.0.0"\ndependencies:\n  flutter:\n    sdk: flutter\n'
for name in primary:
    text += f'  {name}: any\n'
text += '  bridge_flutter:\n    path: ' + str(root / 'rust_builder') + '\n'
text += 'dependency_overrides:\n'
# 当前 package_config 是 pubspec.lock 的实际解析结果，避免另拉一版 native 分支。
for name, path in paths.items():
    if '/.pub-cache/' in path:
        text += f'  {name}:\n    path: {json.dumps(path)}\n'
text += 'flutter:\n  uses-material-design: true\n  assets:\n    - assets/\n'
(app / 'pubspec.yaml').write_text(text)
for relative in ['rust', 'plugin/pl_player/player_port']:
    link = app / 'lib' / relative
    link.parent.mkdir(parents=True, exist_ok=True)
    if not link.exists():
        os.symlink(root / 'lib' / relative, link, target_is_directory=True)
shutil.copy(root / 'tool/player_port_probe/main.dart', app / 'lib/main.dart')
shutil.copy(root / 'tool/player_port_probe/probe_server.dart', app / 'lib/probe_server.dart')
assets = app / 'assets'
assets.mkdir(exist_ok=True)
for name in ['fixture.mp4', 'quality.mp4']:
    shutil.copy(root / 'build/player-port-runtime' / name, assets / name)
shutil.copy(root / 'rust/crates/bili-api/tests/fixtures/danmaku_seg/typical.raw.pb', assets / 'danmaku.pb')
for name in ['DebugProfile.entitlements', 'Release.entitlements']:
    path = app / 'macos/Runner' / name
    content = path.read_text()
    for key in ['com.apple.security.network.server', 'com.apple.security.network.client']:
        if key not in content:
            content = content.replace('</dict>', f'<key>{key}</key><true/>\n</dict>')
    path.write_text(content)
# 沿用产品工程已有的旧插件 compileSdk/JVM 兼容配置，仅限制探针 ABI。
android_build = (root / 'android/build.gradle.kts').read_text()
android_build += """
subprojects {
    tasks.configureEach {
        if (name.startsWith("cargokitCargoBuild")) {
            doFirst { setProperty("targetPlatforms", listOf("android-arm64")) }
        }
    }
}
"""
(app / 'android/build.gradle.kts').write_text(android_build)
activity = app / 'android/app/src/main/kotlin/io/github/kilikili/playerportprobe/kilikili/MainActivity.kt'
shutil.copy(root / 'tool/player_port_probe/MainActivity.kt', activity)
manifest = app / 'android/app/src/main/AndroidManifest.xml'
content = manifest.read_text()
if 'supportsPictureInPicture' not in content:
    content = content.replace('android:name=".MainActivity"', 'android:name=".MainActivity" android:supportsPictureInPicture="true"')
manifest.write_text(content)
# 直接复用产品的 iOS PiP 原生实现，避免探针另写一套视频帧转发。
delegate = (root / 'ios/Runner/AppDelegate.swift').read_text()
delegate = delegate.replace('    application.applicationSupportsShakeToEdit', '    try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .moviePlayback)\n    try? AVAudioSession.sharedInstance().setActive(true)\n    application.applicationSupportsShakeToEdit')
(app / 'ios/Runner/AppDelegate.swift').write_text(delegate + '\n' + (root / 'ios/Runner/PipPlugin.swift').read_text())
plist_path = app / 'ios/Runner/Info.plist'
import plistlib
info = plistlib.loads(plist_path.read_bytes())
info['UIBackgroundModes'] = ['audio']
plist_path.write_bytes(plistlib.dumps(info))
subprocess.run(['flutter', 'pub', 'get', '--offline'], cwd=app, check=True)
print(app)
