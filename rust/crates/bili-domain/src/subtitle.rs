//! 字幕轨实体。
//!
//! 来源有两处，形态不同：`DmView` 的 protobuf（本实体的直接来源）与
//! 视频详情 JSON 里的 `subtitle.subtitles`。基线的 `Subtitle` 类同时
//! 承载两者，于是每个字段都是可空的——而从 protobuf 来的那条路径上，
//! proto3 的零值保证了它们**不会**是 null（`lib/grpc/dm.dart` 的
//! `dmViewToSubtitle` 逐字段取值，取不到就是空串 / 0）。
//!
//! 因此这里是非空字段：可空性应当描述数据的真实形态，而不是沿用一个
//! 为两种来源折中过的类。将来 JSON 那条路径迁过来时，若它确实会缺字段，
//! 再在**那边**的 DTO 里处理。

use serde::Serialize;

/// 一条字幕轨。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Subtitle {
    pub id: i64,
    /// 语言代码，如 `zh-CN`、`ai-zh`。
    pub lan: String,
    /// 语言的展示名。
    pub lan_doc: String,
    /// 字幕 JSON 的地址。
    pub subtitle_url: String,
    /// `SubtitleType` 的线上数值：0 CC（人工）/ 1 AI。
    pub subtitle_type: i32,
    /// 是否 AI 生成。基线按 `type == AI` 判定，不看 `lan` 前缀。
    pub is_ai: bool,
}

/// 字幕文件里的一条。
///
/// 时间是**秒**，浮点——服务端就是这么下发的（`{"from":12.5,"to":14.0}`），
/// 基线也原样拿去格式化。转成整数毫秒再格式化会与基线差 1 毫秒，而
/// 那种差异只会在 golden 里暴露，不会在界面上。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SubtitleCue {
    pub from: f64,
    pub to: f64,
    pub content: String,
}

/// 按基线 `Subtitle.compareTo` 的顺序排序。
///
/// 规则（`lib/models_new/video/video_play_info/subtitle.dart`）：
/// 1. `lan` 含 `zh` 的排前面
/// 2. 其次非 AI 排前面
/// 3. 否则视为相等，**保持原有相对顺序**
///
/// 第 3 条要求排序稳定。Rust 的 `sort_by` 稳定；Dart 的 `List.sort` 只在
/// 元素少于 32 个时稳定（底层走插入排序），超过则是双轴快排。一个视频
/// 的字幕轨是个位数，不会触到那条边界——但如果哪天触到了，差异会表现为
/// 「同语言的两条字幕顺序偶尔不同」，记在这里省得重新查一遍。
pub fn sort_like_baseline(subtitles: &mut [Subtitle]) {
    subtitles.sort_by(|a, b| {
        let a_zh = a.lan.contains("zh");
        let b_zh = b.lan.contains("zh");
        if a_zh != b_zh {
            return if a_zh {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        if a.is_ai != b.is_ai {
            return if a.is_ai {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            };
        }
        std::cmp::Ordering::Equal
    });
}

#[cfg(test)]
mod tests {
    use super::{Subtitle, sort_like_baseline};

    fn track(lan: &str, is_ai: bool, id: i64) -> Subtitle {
        Subtitle {
            id,
            lan: lan.to_owned(),
            lan_doc: String::new(),
            subtitle_url: String::new(),
            subtitle_type: i32::from(is_ai),
            is_ai,
        }
    }

    #[test]
    fn chinese_first_then_non_ai() {
        let mut tracks = vec![
            track("en-US", false, 1),
            track("ai-zh", true, 2),
            track("zh-CN", false, 3),
            track("ai-en", true, 4),
        ];
        sort_like_baseline(&mut tracks);
        let ids: Vec<_> = tracks.iter().map(|t| t.id).collect();
        assert_eq!(ids, [3, 2, 1, 4]);
    }

    #[test]
    fn equal_keys_keep_their_order() {
        let mut tracks = vec![
            track("zh-CN", false, 1),
            track("zh-Hant", false, 2),
            track("zh-HK", false, 3),
        ];
        sort_like_baseline(&mut tracks);
        let ids: Vec<_> = tracks.iter().map(|t| t.id).collect();
        assert_eq!(ids, [1, 2, 3], "同档次的必须保持原顺序");
    }

    #[test]
    fn ai_prefix_alone_does_not_decide() {
        // 判定 AI 看的是 type 字段，不是 lan 前缀。
        let mut tracks = vec![track("ai-zh", false, 1), track("zh-CN", true, 2)];
        sort_like_baseline(&mut tracks);
        assert_eq!(tracks[0].id, 1);
    }
}
