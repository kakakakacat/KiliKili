/// 规范化结果 → 基线模型可消费的形状。
///
/// Rust 取数路径返回的是**规范化**后的 JSON（与
/// `video_detail_normalize.dart` 同形）。要让它走进现有 UI，最省事且最
/// 不易出错的办法是把它还原成接口原始形状，再交给
/// `VideoDetailData.fromJson`——而不是另写一套从规范化 JSON 直接构造模型
/// 的代码。后者等于把模型的构造逻辑抄第二遍，模型一改就会悄悄脱节。
///
/// 绝大多数字段规范化时没有改名，直接透传即可；只有三处被基线模型压扁
/// 过，需要还原回嵌套形状。
///
/// 本文件**不得** import flutter 或 hive：它要能被纯 dart 的测试与工具
/// 使用。
library;

/// 把规范化结果还原成 `VideoDetailData.fromJson` 认得的形状。
///
/// 已知的无害差异：当 `is_stein_gate` / `argue_msg` 缺失时，这里仍会造出
/// 一个字段全空的 `rights` / `argue_info` 对象，而原始响应里这两个键可能
/// 整个不存在。模型只读取其中那一个字段，二者等价。
Map<String, dynamic> denormalizeVideoDetail(Map<String, dynamic> n) {
  return {
    ...n,
    'rights': {'is_stein_gate': n['is_stein_gate']},
    'argue_info': {'argue_msg': n['argue_msg']},
    'staff': (n['staff'] as List<dynamic>?)
        ?.map(
          (s) => {
            ...s as Map<String, dynamic>,
            'vip': {'type': s['vip_type'], 'status': s['vip_status']},
            'official': {
              'type': s['official_type'],
              'desc': s['official_desc'],
            },
          },
        )
        .toList(),
  };
}
