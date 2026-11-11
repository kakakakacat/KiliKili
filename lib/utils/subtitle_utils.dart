import 'dart:typed_data' show Uint8List;

import 'package:kilikili/common/constants.dart';
import 'package:kilikili/http/init.dart';
import 'package:kilikili/models/common/enum_with_label.dart';
import 'package:kilikili/utils/accounts/account.dart';
import 'package:kilikili/utils/extension/string_ext.dart';
import 'package:kilikili/utils/path_utils.dart';
import 'package:kilikili/utils/subtitle_format.dart' as subtitle_format;
import 'package:dio/dio.dart' show Options;

enum SubtitleFormat implements EnumWithLabel {
  json('JSON'),
  vtt('WEBVTT'),
  srt('SRT');

  @override
  final String label;
  const SubtitleFormat(this.label);
}

abstract final class SubtitleUtils {
  static const String _json = '.json';
  static const String _bcc = '.bcc';
  static bool isJson(String fileName) =>
      fileName.endsWith(_json) ||
      fileName.endsWith(_bcc) ||
      fileName.endsWith(PathUtils.subtitleExt);

  static const exts = [_json, 'vtt', 'srt', 'ass', _bcc, PathUtils.subtitleExt];

  /// 转换实现搬到了 `subtitle_format.dart`（纯 dart，无 flutter/dio 依赖），
  /// 以便 fixture 生成器直接用**基线自己**产出 golden，与 Rust 侧
  /// `bili-core::subtitle_format` 对照。这里保留入口，调用方不受影响。
  static String json2Vtt(List list) => subtitle_format.json2Vtt(list);

  static String json2Srt(List list) => subtitle_format.json2Srt(list);

  static Future<Uint8List?> getSubtitle(String url) async {
    final res = await Request.dio.get<Uint8List>(
      url.http2https,
      options: Options(
        responseType: .bytes,
        headers: Constants.baseHeaders,
        extra: {'account': const NoAccount()},
      ),
    );
    if (res.statusCode != 200) return null;
    return Uint8List.fromList(
      Request.responseBytesDecoder(res.data!, res.headers.map),
    );
  }
}
