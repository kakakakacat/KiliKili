import 'dart:io';
import 'dart:typed_data';

import 'package:archive/archive.dart';
import 'package:flutter_test/flutter_test.dart';

// oracle 逐行保留迁移前 GrpcReq.decompressProtobuf 的行为（2705b39d6）。
// 不 import lib/；相同原始帧的 Rust 输出由 grpc_fetch.rs 对照同一 payload。
Uint8List _baseline(Uint8List frame) {
  final length = ByteData.sublistView(frame, 1, 5).getInt32(0, Endian.big);
  final body = Uint8List.sublistView(frame, 5, length + 5);
  return frame[0] == 1
      ? Uint8List.fromList(const GZipDecoder().decodeBytes(body))
      : body;
}

void main() {
  test('真实脱敏帧的 Dart 与 Rust 载荷逐字节一致', () {
    const dir = 'rust/crates/bili-api/tests/fixtures/grpc';
    expect(
      _baseline(File('$dir/dm_segment.frame.bin').readAsBytesSync()),
      File('$dir/dm_segment.payload.pb').readAsBytesSync(),
    );
  });
}
