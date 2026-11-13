import 'dart:convert' show jsonEncode;

// 这里仅封装 WebView 的 SDK 与消息桥接；验证码注册及重试属于登录切片 #65。
String buildGeetestHtml(String gt, String challenge, {bool linux = false}) {
  final ts = DateTime.now().millisecondsSinceEpoch;
  final config = jsonEncode({
    'gt': gt,
    'challenge': challenge,
    'offline': false,
    'new_captcha': true,
    'product': 'bind',
    'width': '100%',
    'https': true,
    'protocol': 'https://',
  }).replaceAll('<', r'\u003c');
  final bridge = linux
      ? "window.KiliKili.postMessage(n+':'+JSON.stringify(o))"
      : 'window.flutter_inappwebview.callHandler(n,o)';
  final bridgeAvailable = linux
      ? 'window.KiliKili'
      : 'window.flutter_inappwebview&&window.flutter_inappwebview.callHandler';

  return '''<!DOCTYPE html><html><head>
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta charset="utf-8"></head>
<style>#E{position:fixed;inset:0;display:flex;align-items:center;justify-content:center;color:#555;pointer-events:none}</style>
<body><div id="E">正在加载验证码…</div>
<script>
var Q=[];
function flush(){
  if(!($bridgeAvailable))return;
  while(Q.length){var item=Q.shift(),n=item[0],o=item[1];$bridge;}
}
function R(n,o){Q.push([n,o]);flush();}
window.addEventListener('flutterInAppWebViewPlatformReady',flush);
function E(o){document.getElementById('E').textContent='验证码加载失败，请关闭后重试或使用扫码登录';R('error',o);}
window.addEventListener('error',function(){E('geetest script error');});
window.addEventListener('unhandledrejection',function(){E('geetest script rejected');});
var C,S,T,t;
T=function(){if(C&&S&&!t){
  try{
    t=Geetest(C).onSuccess(function(){R('success',t.getValidate());})
      .onError(E).onClose(function(){R('close',null);});
    t.onReady(function(){document.getElementById('E').textContent='';R('ready',null);t.verify();});
  }catch(e){E('geetest initialization failed');}
}};
function geetest_$ts(d){
  if(!d||d.status!='success'){E(d);return;}
  C=Object.assign($config,d.data);T();
}
function G(){S=1;T();}
</script>
<script src="https://static.geetest.com/static/js/fullpage.0.0.0.js" onload="G()" onerror="E('geetest script load failed')"></script>
<script src="https://api.geetest.com/gettype.php?gt=${Uri.encodeQueryComponent(gt)}&callback=geetest_$ts" onerror="E('geetest config load failed')"></script>
</body></html>''';
}
