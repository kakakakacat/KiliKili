import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models_new/video/video_play_info/subtitle.dart';
import 'package:kilikili/rust/api/grpc.dart' as api;
import 'package:kilikili/services/rust/rust_grpc.dart';

abstract final class RustDmView {
  static Future<LoadingState<List<Subtitle>>> fetchSubtitles(
    int aid,
    int cid,
  ) async {
    try {
      final result = await api.fetchDmSubtitles(
        context: RustGrpc.context(),
        aid: aid,
        cid: cid,
      );
      final data = result.data;
      if (data == null) {
        return Error(result.errorDetail ?? '字幕请求失败', code: result.errorCode);
      }
      // Subtitle 仍被视频详情 JSON 路径消费；这里只做字段适配，不保留排序/解析。
      // 随该 JSON 来源迁为统一字幕实体时删除，避免本切片扩张到视频详情模型。
      return Success(
        data
            .map(
              (s) => Subtitle(
                id: s.id,
                lan: s.lan,
                lanDoc: s.lanDoc,
                subtitleUrl: s.subtitleUrl,
                type: s.subtitleType,
                isAi: s.isAi,
              ),
            )
            .toList(growable: false),
      );
    } catch (_) {
      return const Error('Rust 字幕桥接不可用');
    }
  }
}
