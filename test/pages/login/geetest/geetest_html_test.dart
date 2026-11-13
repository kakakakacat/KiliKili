import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kilikili/pages/login/geetest/geetest_html.dart';

// 用 Node 的隔离上下文执行实际页面脚本，不访问极验，也不依赖打过补丁的 Widget。
const harness = r'''
const assert = require('node:assert/strict');
const vm = require('node:vm');
const {html, scenario, linux} = JSON.parse(process.argv[1]);
const events = [];
const listeners = {};
const label = {textContent: ''};
let ready, success, error;
const captcha = {
  onSuccess(cb) { success = cb; return this; },
  onError(cb) { error = cb; return this; },
  onClose() { return this; },
  onReady(cb) { ready = cb; return this; },
  verify() {},
  getValidate() { return {geetest_validate:'validate',geetest_seccode:'seccode',geetest_challenge:'challenge'}; }
};
const context = {
  document: {getElementById() {return label;}},
  addEventListener(name, cb) { listeners[name] = cb; },
  Geetest() { return captcha; },
  console,
};
context.window = context;
if (linux) context.KiliKili = {postMessage(msg) { const i=msg.indexOf(':'); events.push([msg.slice(0,i),JSON.parse(msg.slice(i+1))]); }};
else if (scenario !== 'late bridge') context.flutter_inappwebview = {callHandler(...args) {events.push(args);}};
vm.createContext(context);
for (const match of html.matchAll(/<script>([\s\S]*?)<\/script>/g)) vm.runInContext(match[1], context);
const callback = Object.keys(context).find(key => /^geetest_\d+$/.test(key));
context[callback]({status:'success',data:{type:'fullpage'}});
context.G();
if (scenario === 'SDK error') {
  error({code:'error_02',msg:'old challenge'});
  assert.match(label.textContent, /失败/);
  assert.equal(events.at(-1)[0], 'error');
} else if (scenario === 'ready') {
  ready();
  assert.ok(events.some(event => event[0] === 'ready'));
} else if (scenario === 'late bridge') {
  success();
  assert.equal(events.length, 0);
  context.flutter_inappwebview = {callHandler(...args) {events.push(args);}};
  listeners.flutterInAppWebViewPlatformReady();
  assert.equal(events.at(-1)[0], 'success');
  assert.equal(events.at(-1)[1].geetest_validate, 'validate');
} else if (scenario === 'script error') {
  listeners.error({message:'script failed'});
  assert.match(label.textContent, /失败/);
  assert.equal(events.at(-1)[0], 'error');
} else if (scenario === 'config error') {
  context[callback]({status:'error',error_code:'error_34'});
  assert.match(label.textContent, /失败/);
  assert.equal(events.at(-1)[0], 'error');
}
''';

void main() {
  for (final linux in [false, true]) {
    for (final scenario in [
      'SDK error',
      'ready',
      if (!linux) 'late bridge',
      'script error',
      'config error',
    ]) {
      test('${linux ? 'Linux' : 'WebView'}: $scenario', () async {
        final result = await Process.run('node', [
          '-e',
          harness,
          jsonEncode({
            'html': buildGeetestHtml('gt', 'challenge', linux: linux),
            'scenario': scenario,
            'linux': linux,
          }),
        ]);
        expect(
          result.exitCode,
          0,
          reason: '${result.stdout}\n${result.stderr}',
        );
      });
    }
  }
}
