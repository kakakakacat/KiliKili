import 'dart:convert';
import 'dart:typed_data';

import 'package:kilikili/grpc/bilibili/rpc.pb.dart';
import 'package:kilikili/http/constants.dart';
import 'package:kilikili/http/init.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:archive/archive.dart';
import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart' show kDebugMode, compute;
import 'package:protobuf/protobuf.dart' show GeneratedMessage;

abstract final class GrpcReq {
  static const _isolateSize = 256 * 1024;
  static const _gzipMinLength = 64;

  static final options = Options(
    contentType: 'application/grpc',
    responseType: ResponseType.bytes,
  );

  static Uint8List compressProtobuf(Uint8List proto) {
    final compress = proto.length > _gzipMinLength;
    if (compress) {
      proto = const GZipEncoder().encodeBytes(proto);
    }
    return Uint8List(5 + proto.length)
      ..[0] = compress ? 1 : 0
      ..buffer.asByteData(1, 4).setInt32(0, proto.length, Endian.big)
      ..setAll(5, proto);
  }

  static Uint8List decompressProtobuf(Uint8List data) {
    final length = ByteData.sublistView(data, 1, 5).getInt32(0, Endian.big);

    if (data[0] == 1) {
      return Request.gzip.convert(Uint8List.sublistView(data, 5, length + 5))
          as Uint8List;
    } else {
      return Uint8List.sublistView(data, 5, length + 5);
    }
  }

  static LoadingState<T> _parse<T>((Uint8List, T Function(Uint8List)) args) {
    try {
      final data = decompressProtobuf(args.$1);
      final grpcResponse = args.$2(data);
      return Success(grpcResponse);
    } catch (e) {
      return Error(e.toString());
    }
  }

  static Future<LoadingState<T>> request<T extends GeneratedMessage>(
    String url,
    GeneratedMessage request,
    T Function(Uint8List) grpcParser, {
    bool isolate = false,
  }) async {
    final frames = await _post(url, request);
    if (frames case Success(:final response)) {
      return isolate && response.length > _isolateSize
          ? compute(_parse, (response, grpcParser))
          : _parse((response, grpcParser));
    }
    // Error / Loading 都是 LoadingState<Never>，对任何 T 都成立。
    return frames as LoadingState<T>;
  }

  /// 只取回**解帧并解压后**的 protobuf 字节，不在 Dart 侧解析。
  ///
  /// 给 Rust 解析路径用（见 `lib/services/rust/rust_danmaku.dart`）：它要
  /// 的是字节，而 [request] 的产物是解析好的对象，再 `writeToBuffer` 一次
  /// 等于把要省的那次解析又做了一遍。
  ///
  /// 解帧与解压留在 Dart：它们属于传输外壳，而传输这条链路还没迁移
  /// （见 `rust/crates/bili-api/src/danmaku.rs` 的模块文档）。等 Rust 接管
  /// 传输时，这一步一并过去。
  static Future<LoadingState<Uint8List>> requestRaw(
    String url,
    GeneratedMessage request,
  ) async {
    final frames = await _post(url, request);
    if (frames case Success(:final response)) {
      try {
        return Success(decompressProtobuf(response));
      } catch (e) {
        return Error(e.toString());
      }
    }
    return frames;
  }

  /// 发请求并处理 gRPC 状态，返回**未解帧**的响应体。
  ///
  /// [request] 与 [requestRaw] 的共同部分。抽出来是为了让两条路径的
  /// 错误处理（业务错误体、`Grpc-Status`、`Grpc-Status-Details-Bin`）
  /// 只有一份——两份迟早会漂移，而漂移的表现是同一个服务端故障在两条
  /// 路径上给出不同的提示。
  static Future<LoadingState<Uint8List>> _post(
    String url,
    GeneratedMessage request,
  ) async {
    final response = await Request().post<Uint8List>(
      HttpString.appBaseUrl + url,
      data: compressProtobuf(request.writeToBuffer()),
      options: options,
    );

    if (response.data case final Map map) {
      return Error(map['message']);
    }

    if (response.headers.value('Grpc-Status') == '0') {
      final data = response.data;
      if (data is Uint8List) {
        return Success(data);
      } else {
        return Error('grpc: ${data.runtimeType} is not Uint8List');
      }
    } else {
      try {
        int? code;
        String msg = response.headers.value('Grpc-Status-Details-Bin') ?? '';
        if (msg.isNotEmpty) {
          final padding = -msg.length & 3;
          if (padding != 0) {
            msg += '=' * padding;
          }
          final msgBytes = base64Decode(msg);
          try {
            final grpcMsg = Status.fromBuffer(msgBytes);
            final details = grpcMsg.details
                .map((e) => Status.fromBuffer(e.value))
                .toList();
            code = details.firstOrNull?.code;
            // UNKNOWN : -400 : msg
            final errMsg = details.map((e) => e.message).join('\n');
            msg = kDebugMode
                ? 'CODE: ${grpcMsg.code}(${grpcMsg.message})\n'
                      'MSG: $errMsg'
                : errMsg;
          } catch (e) {
            msg = utf8.decode(msgBytes, allowMalformed: true);
          }
        }
        return Error(msg, code: code);
      } catch (e) {
        return Error(e.toString());
      }
    }
  }

  // static Future playerOnline({
  //   int aid = 0,
  //   int cid = 0,
  // }) {
  //   return _request(
  //       GrpcUrl.playerOnline,
  //       PlayerOnlineReq(aid: Int64(aid), cid: Int64(cid), playOpen: true),
  //       PlayerOnlineReply.fromBuffer,
  //       onSuccess: (response) => response.totalNumberText);
  // }

  // static Future popular(int idx) {
  //   return _request(GrpcUrl.popular, PopularResultReq(idx: Int64(idx)),
  //       PopularReply.fromBuffer, onSuccess: (response) {
  //     response.items.retainWhere((item) => item.smallCoverV5.base.goto == 'av');
  //     return {'status': true, 'data': response.items};
  //   });
  // }
}
