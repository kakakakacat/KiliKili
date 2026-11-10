/// 规范化结果 → 基线模型可消费的形状。
///
/// 理由同 `video_detail_denormalize.dart`：把规范化结果还原成接口原始
/// 形状再交给 `PlayUrlModel.fromJson`，而不是另写一套从规范化 JSON 直接
/// 构造模型的代码——后者等于把模型的构造逻辑抄第二遍，模型一改就会悄悄
/// 脱节。清晰度描述、音轨排序与「（AI）」后缀也因此仍由模型产出。
///
/// 规范化时改了名的只有五处，其余透传。
///
/// 本文件**不得** import flutter 或 hive：要能被纯 dart 的测试使用。
library;

/// 把规范化结果还原成 `PlayUrlModel.fromJson` 认得的形状。
///
/// 已知的无害差异：音轨已被合并进 `audio`（规范化时按 flac → dolby →
/// 普通的顺序拼好），还原时不再拆回 `flac`/`dolby` 三处。模型只是按顺序
/// 读 `audio`，两者等价。
Map<String, dynamic> denormalizePlayUrl(Map<String, dynamic> n) {
  final dash = n['dash'] as Map<String, dynamic>?;
  return {
    ...n,
    'timelength': n['time_length'],
    'support_formats': (n['support_formats'] as List?)
        ?.map(
          (f) => {
            ...f as Map<String, dynamic>,
            'new_description': f['new_desc'],
          },
        )
        .toList(),
    'dash': dash == null
        ? null
        : {
            'duration': dash['duration'],
            'minBufferTime': dash['min_buffer_time'],
            'video': (dash['video'] as List?)?.map(_stream).toList(),
            'audio': (dash['audio'] as List?)?.map(_stream).toList(),
          },
    'clip_info_list': (n['clip_info_list'] as List?)
        ?.map(
          (c) => {
            ...c as Map<String, dynamic>,
            'clipType': c['clip_type'],
          },
        )
        .toList(),
  };
}

/// 模型认 `bandWidth` 与 `bandwidth` 两种写法，但不认 `band_width`。
/// 其余键名（`base_url`、`backup_url`、`frame_rate`、`start_with_sap`、
/// `segment_base`）模型都已接受，原样透传。
Map<String, dynamic> _stream(dynamic s) => {
  ...s as Map<String, dynamic>,
  'bandwidth': s['band_width'],
};
