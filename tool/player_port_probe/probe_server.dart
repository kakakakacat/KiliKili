import 'dart:io';
import 'dart:typed_data';

/// 本地限速 Range 资源；blocked 只停传输，让后端自己报告缓冲事实。
class ControlledMediaServer {
  ControlledMediaServer._(this._server, this._bytes) {
    _server.listen(_serve);
  }
  static Future<ControlledMediaServer> start(File file) async =>
      ControlledMediaServer._(
        await HttpServer.bind(InternetAddress.loopbackIPv4, 0),
        await file.readAsBytes(),
      );
  final HttpServer _server;
  final Uint8List _bytes;
  bool blocked = false;
  bool _stopped = false;
  int requests = 0;
  String get url => 'http://127.0.0.1:${_server.port}/quality.mp4';
  Future<void> _serve(HttpRequest request) async {
    requests++;
    final range = RegExp(r'bytes=(\d+)-(\d*)')
        .firstMatch(request.headers.value('range') ?? '');
    final start = range == null ? 0 : int.parse(range[1]!);
    final end = range == null || range[2]!.isEmpty
        ? _bytes.length - 1
        : int.parse(range[2]!);
    final response = request.response;
    response.headers.set('Accept-Ranges', 'bytes');
    response.headers.contentType = ContentType('video', 'mp4');
    if (range != null) {
      response.statusCode = HttpStatus.partialContent;
      response.headers.set(
        'Content-Range',
        'bytes $start-$end/${_bytes.length}',
      );
    }
    response.contentLength = end - start + 1;
    try {
      for (var offset = start; offset <= end && !_stopped; offset += 16384) {
        while (blocked && !_stopped) {
          await Future<void>.delayed(const Duration(milliseconds: 20));
        }
        if (_stopped) break;
        response.add(
          _bytes.sublist(offset, (offset + 16384).clamp(0, end + 1)),
        );
        await response.flush();
        await Future<void>.delayed(const Duration(milliseconds: 10));
      }
      await response.close();
    } on SocketException {
      // 替换资源会主动断开旧请求；不把探针连接的正常结束当成播放器失败。
    } on HttpException {
      // 同上。
    }
  }

  Future<void> close() async {
    _stopped = true;
    blocked = false;
    await _server.close(force: true);
  }
}
