import 'dart:collection';
import 'dart:io' show File;
import 'dart:typed_data';

import 'package:kilikili/grpc/bilibili/community/service/dm/v1.pb.dart';
import 'package:kilikili/grpc/dm.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/pages/danmaku/engine.dart';
import 'package:kilikili/plugin/pl_player/controller.dart';
import 'package:kilikili/plugin/pl_player/models/data_source.dart';
import 'package:kilikili/services/rust/rust_danmaku.dart';
import 'package:kilikili/utils/accounts.dart';
import 'package:kilikili/utils/danmaku_utils.dart';
import 'package:kilikili/utils/path_utils.dart';
import 'package:kilikili/utils/utils.dart';
import 'package:path/path.dart' as path;

class PlDanmakuController {
  PlDanmakuController(this._cid, this._plPlayerController, this._isFileSource)
    : _mergeDanmaku = _plPlayerController.mergeDanmaku {
    // 开关关闭、Rust 未就绪、或用户的正则规则 Rust 编不过时返回 null，
    // 整条走下面的 Dart 原路径。
    _rust = RustDanmakuSession.open(
      isLogin: _isLogin,
      midHash: _plPlayerController.midHash,
      mergeDanmaku: _mergeDanmaku,
      filters: _plPlayerController.filters,
    );
  }

  final int _cid;
  final PlPlayerController _plPlayerController;
  final bool _mergeDanmaku;
  final bool _isFileSource;

  late final _isLogin = Accounts.main.isLogin;

  /// Rust 弹幕引擎会话。非 null 时，解码 / 合并 / 过滤 / 分桶都在 Rust，
  /// [_dmSegMap] 不参与；运行中任何一步失败都会 [_degradeToDart]。
  RustDanmakuSession? _rust;

  final Map<int, List<DanmakuElem>> _dmSegMap = HashMap();
  // 已请求的段落标记
  late final Set<int> _requestedSeg = HashSet();

  /// 已**并入** [_dmSegMap] 的分段。
  ///
  /// 与 [_requestedSeg] 的区别是记录时机：那个在请求**前**记上，用来避免
  /// 重复请求；这个在并入**后**才记，用来避免同一分段被并入两次——
  /// [_degradeToDart] 会清空请求记录，此时在途的那份响应回来，可能与重新
  /// 发出的那份撞在一起，两份都并入就是同一段弹幕飘两遍。
  ///
  /// Rust 侧有对应的闸门（`DanmakuSession::ingest_segment` 的
  /// `already_loaded`）。
  final Set<int> _loadedSeg = HashSet();

  void dispose() {
    _dmSegMap.clear();
    _requestedSeg.clear();
    _loadedSeg.clear();
    // 不关就是泄漏一个视频的全部弹幕，而泄漏在界面上没有任何症状。
    _rust?.close();
    _rust = null;
  }

  Future<void> queryDanmaku(int segmentIndex) async {
    if (_isFileSource) {
      return;
    }
    if (_requestedSeg.contains(segmentIndex)) {
      return;
    }
    _requestedSeg.add(segmentIndex);

    if (_rust != null) {
      // Rust 路径要的是字节：在 Dart 侧解一遍再 writeToBuffer 交过去，
      // 等于把要省的那次解析又做了一遍。
      final res = await DmGrpc.dmSegMobileRaw(
        cid: _cid,
        segmentIndex: segmentIndex + 1,
      );
      if (res case Success(:final response)) {
        await _handleSegmentBytes(segmentIndex, response);
      } else {
        _requestedSeg.remove(segmentIndex);
      }
      return;
    }

    final res = await DmGrpc.dmSegMobile(
      cid: _cid,
      segmentIndex: segmentIndex + 1,
    );

    if (res case Success(:final response)) {
      if (response.state == 1) {
        _plPlayerController.dmState.add(_cid);
      }
      handleDanmaku(response.elems);
    } else {
      _requestedSeg.remove(segmentIndex);
    }
  }

  /// 把一个分段的原始 protobuf 字节交给 Rust 引擎，失败则就地降级回 Dart。
  ///
  /// 降级时用的是**同一份字节**：网络已经花过了，没有理由为了换条解析
  /// 路径再请求一次。
  Future<void> _handleSegmentBytes(int segmentIndex, Uint8List bytes) async {
    final rust = _rust;
    if (rust != null) {
      final result = await rust.pushSegment(segmentIndex, bytes);
      if (result != null) {
        if (result.state == 1) {
          _plPlayerController.dmState.add(_cid);
        }
        return;
      }
      _degradeToDart();
    }

    // 这一段已经并入过（见 [_loadedSeg]），再并一次就是弹幕飘两遍。
    if (_loadedSeg.contains(segmentIndex)) return;

    try {
      final reply = DmSegMobileReply.fromBuffer(bytes);
      if (reply.state == 1) {
        _plPlayerController.dmState.add(_cid);
      }
      handleDanmaku(reply.elems);
      _loadedSeg.add(segmentIndex);
      // _degradeToDart 清空过请求记录，这一段刚刚解完，补回去。
      _requestedSeg.add(segmentIndex);
    } catch (e, s) {
      _requestedSeg.remove(segmentIndex);
      Utils.reportError(e, s);
    }
  }

  /// Rust 引擎出了问题：关掉会话，此后整条走 Dart。
  ///
  /// 已并入 Rust 的分段随会话一起没了，所以清空请求记录，让它们在播放
  /// 推进时按 Dart 路径重新取一遍。多几次请求换一条完整的弹幕轨道，比
  /// 在视频中间留下几段「没人发弹幕」好——后者没有任何提示。
  void _degradeToDart() {
    _rust?.close();
    _rust = null;
    _requestedSeg.clear();
  }

  /// Dart 原路径的分段处理。顺序见 `bucketDanmaku`。
  void handleDanmaku(List<DanmakuElem> elems) {
    bucketDanmaku(
      elems,
      into: _dmSegMap,
      isLogin: _isLogin,
      midHash: _plPlayerController.midHash,
      mergeDanmaku: _mergeDanmaku,
      filters: _plPlayerController.filters,
    );
  }

  List<DanmakuElem>? getCurrentDanmaku(int progress) {
    if (_isFileSource) {
      initFileDmIfNeeded();
    } else {
      final int segmentIndex = DmUtils.calcSegment(progress);
      if (!_requestedSeg.contains(segmentIndex)) {
        queryDanmaku(segmentIndex);
        return null;
      }
    }
    final rust = _rust;
    if (rust != null) {
      return rust.bucket(progress);
    }
    return _dmSegMap[progress ~/ 100];
  }

  bool _fileDmLoaded = false;

  void initFileDmIfNeeded() {
    if (_fileDmLoaded) return;
    _fileDmLoaded = true;
    _initFileDm();
  }

  @pragma('vm:notify-debugger-on-exception')
  Future<void> _initFileDm() async {
    try {
      final file = File(
        path.join(
          (_plPlayerController.dataSource as FileSource).dir,
          PathUtils.danmakuName,
        ),
      );
      if (!file.existsSync()) return;
      final bytes = await file.readAsBytes();
      if (bytes.isEmpty) return;
      // 本地弹幕文件就是一个分段的字节，与网络拿到的同形。
      await _handleSegmentBytes(0, bytes);
    } catch (e, s) {
      Utils.reportError(e, s);
    }
  }
}
