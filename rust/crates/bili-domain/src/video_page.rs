//! 播放页的附属只读接口：AI 总结、在线人数、预览缩略图、播放信息、笔记列表。
//!
//! 这五条在基线里都散在 `lib/http/video.dart`，彼此没有共享模型，唯一的
//! 共同点是「同一个播放页用得到」。放一个模块是为了让切片边界与文件边界
//! 重合（S3 第二批，issue #53），不是因为它们在领域上是一回事。
//!
//! 实体一律 `Option`：这几条接口都会在不同场景下少给字段（没有 AI 总结、
//! 没有字幕、没有分段看点），缺字段不该让整页打不开。

use serde::Serialize;

/// AI 视频总结。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AiConclusion {
    pub summary: Option<String>,
    pub outline: Option<Vec<AiOutline>>,
}

/// 总结里的一段。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AiOutline {
    pub title: Option<String>,
    pub part_outline: Option<Vec<AiOutlinePart>>,
}

/// 段里的一条，`timestamp` 是秒。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AiOutlinePart {
    pub timestamp: Option<i64>,
    pub content: Option<String>,
}

/// 进度条预览缩略图（雪碧图）。
///
/// `images` 里每张图含 `img_x_len * img_y_len` 格，`index` 是每一格对应的
/// 时间点（秒）。两者都必须非空才有意义，见 `bili-api` 侧的说明。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VideoShot {
    pub pvdata: Option<String>,
    pub img_x_len: i64,
    pub img_y_len: i64,
    pub img_x_size: f64,
    pub img_y_size: f64,
    /// 已统一成 https，见 [`crate::video_page::to_https`]。
    pub images: Vec<String>,
    pub index: Vec<i64>,
}

/// 播放信息：续播位置、字幕、分段看点、互动视频入口。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlayInfo {
    pub last_play_cid: Option<i64>,
    /// 原样顺序。基线在构造模型时就按「中文优先、AI 字幕靠后」排了序，
    /// 那是界面偏好而非协议，留给 Dart（ADR-011 决策二）。
    pub subtitles: Option<Vec<PlaySubtitle>>,
    pub view_points: Option<Vec<ViewPoint>>,
    pub interaction: Option<Interaction>,
}

/// 一条字幕。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlaySubtitle {
    pub id: Option<i64>,
    pub lan: Option<String>,
    /// 服务端给的语言名。基线会在后面补「（AI）」，那是界面措辞，不在这里。
    pub lan_doc: Option<String>,
    pub subtitle_url: Option<String>,
    /// 字幕类型。`1` 表示 AI 生成。
    pub kind: Option<i64>,
}

/// 分段看点。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ViewPoint {
    /// 看点类型。基线只在 `type == 2` 时画分段进度条。
    pub kind: Option<i64>,
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub content: Option<String>,
    /// 响应里是小驼峰的 `imgUrl`，不是 `img_url`。
    pub img_url: Option<String>,
}

/// 互动视频（剧情树）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Interaction {
    pub graph_version: Option<i64>,
    pub history_node: Option<HistoryNode>,
}

/// 上次停在剧情树的哪个节点。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HistoryNode {
    pub node_id: Option<i64>,
    pub title: Option<String>,
    pub cid: Option<i64>,
}

/// 视频笔记列表的一页。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoteList {
    /// 总条数，分页判断末页用。
    pub total: Option<i64>,
    pub list: Option<Vec<NoteItem>>,
}

/// 一条笔记。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoteItem {
    pub cvid: Option<i64>,
    pub summary: Option<String>,
    /// 服务端直接给的展示用时间串（如「2024-01-02」），不是时间戳。
    pub pubtime: Option<String>,
    pub author: Option<NoteAuthor>,
}

/// 笔记作者。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoteAuthor {
    pub mid: Option<i64>,
    pub name: Option<String>,
    pub face: Option<String>,
    pub level: Option<i64>,
    /// 硬核会员标记，基线按 `== 1` 判断。
    pub is_senior_member: Option<i64>,
    pub vip_type: Option<i64>,
    pub vip_status: Option<i64>,
    pub vip_label_text: Option<String>,
    pub pendant_image: Option<String>,
    pub official_type: Option<i64>,
    pub official_desc: Option<String>,
}

/// 把协议里混用的 `http://` 与协议相对地址统一成 https。
///
/// 对应基线 `lib/utils/extension/string_ext.dart` 的 `http2https`：
/// 正则 `^(http:)?//` 替换成 `https://`，大小写不敏感，只替换一次。
///
/// 放领域层而不是各处自己拼：缩略图、封面、头像都走这一条规则，散开写迟早
/// 有一处漏掉，而漏掉的表现是「这张图在部分机型上加载不出来」。
#[must_use]
pub fn to_https(url: &str) -> String {
    let rest = if let Some(rest) = strip_prefix_ignore_case(url, "http://") {
        rest
    } else if let Some(rest) = strip_prefix_ignore_case(url, "//") {
        rest
    } else {
        return url.to_owned();
    };
    format!("https://{rest}")
}

fn strip_prefix_ignore_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &s[prefix.len()..])
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::to_https;

    #[test]
    fn http_and_protocol_relative_both_become_https() {
        assert_eq!(
            to_https("http://i0.example.invalid/a.jpg"),
            "https://i0.example.invalid/a.jpg"
        );
        assert_eq!(
            to_https("//i0.example.invalid/a.jpg"),
            "https://i0.example.invalid/a.jpg"
        );
    }

    #[test]
    fn https_is_left_alone() {
        assert_eq!(
            to_https("https://i0.example.invalid/a.jpg"),
            "https://i0.example.invalid/a.jpg"
        );
    }

    #[test]
    fn match_is_case_insensitive_like_the_baseline_regexp() {
        assert_eq!(to_https("HTTP://a/b"), "https://a/b");
    }

    #[test]
    fn only_the_leading_occurrence_is_replaced() {
        // 基线用 replaceFirst 且正则锚在开头，URL 参数里的 http:// 不动。
        assert_eq!(
            to_https("http://a/b?next=http://c"),
            "https://a/b?next=http://c"
        );
    }

    #[test]
    fn a_non_url_is_returned_unchanged() {
        assert_eq!(to_https(""), "");
        assert_eq!(to_https("ht"), "ht");
        assert_eq!(to_https("ftp://a"), "ftp://a");
    }
}
