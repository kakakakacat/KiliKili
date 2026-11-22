//! 列表数据：**合成条数，真实管线**。
//!
//! 每一行都是这样来的：
//!
//! ```text
//!   pb::ReplyInfo（按 reply_grpc_golden.rs 的形状合成）
//!     → prost 编码
//!     → bili_api::grpc_reply::project_main_list   ← 生产投影，一字没改
//!     → bili_domain::reply_grpc::ReplyEntity
//!     → bili_domain::reply_richtext::segment      ← 生产分词，一字没改
//! ```
//!
//! 为什么不直接 `ReplyEntity { .. }` 写字面量：这个 spike 要回答的是
//! 「真实评论的段数、配图数、昵称长度摆进列表之后还剩多少帧」。投影与分词
//! 的输出形状（一条正文被切成几段、哪些段带图）是由生产代码决定的，手写
//! 实体等于把被测对象换成了自己的假设。
//!
//! **条数是合成的**：`fixtures/reply_grpc/main_list.rust.json` 只有三条，
//! 压不动列表。记录里会写明这一点——别把这里的 20000 条当成线上的一页。

use std::collections::BTreeMap;
use std::time::Instant;

use anyhow::{Context, Result};
use bili_api::grpc_reply::{MainListOutcome, project_main_list};
use bili_domain::reply_filter::{ReplyFilter, ReplyFilterSettings};
use bili_domain::reply_grpc::ReplyEntity;
use bili_domain::reply_richtext::{
    ReplyRichTextInput, ReplyRichTextSettings, ReplySegment, segment,
};
use bili_grpc::bilibili::main::community::reply::v1 as pb;
use prost::Message;

/// 一条评论在列表里要显示的东西，外加它的分词结果。
pub struct Row {
    pub entity: ReplyEntity,
    pub segments: Vec<ReplySegment>,
}

pub struct Built {
    pub rows: Vec<Row>,
    /// 合成 + 编码 pb 的耗时（毫秒）。
    pub synth_ms: f32,
    /// 生产投影的耗时（毫秒）。
    pub project_ms: f32,
    /// 生产分词的耗时（毫秒）。
    pub segment_ms: f32,
    pub encoded_bytes: usize,
}

/// 八种形状轮着来。真实评论区就是这样混着的：一条「前排」后面跟一条
/// 三百字小作文，中间夹着带四张图的和只有一个表情的。
const SHAPES: usize = 8;

/// 合成 `count` 条评论并跑完整条生产管线。
///
/// # Errors
/// 投影失败（pb 编得不对）或过滤规则编不过。
pub fn build(count: usize, video_duration_ms: i64) -> Result<Built> {
    let started = Instant::now();
    let mut replies = Vec::with_capacity(count);
    for index in 0..count {
        replies.push(reply(index));
    }
    let page = pb::MainListReply {
        replies,
        subject_control: Some(pb::SubjectControl {
            up_mid: Some(3001),
            count: Some(i64::try_from(count).unwrap_or(i64::MAX)),
            title: Some("全部评论".to_owned()),
            show_title: Some(true),
            ..pb::SubjectControl::default()
        }),
        cursor: Some(pb::CursorReply {
            is_end: Some(true),
            ..pb::CursorReply::default()
        }),
        ..pb::MainListReply::default()
    };
    let bytes = page.encode_to_vec();
    let synth_ms = elapsed_ms(started);

    let filter = ReplyFilter::compile(&ReplyFilterSettings::default())
        .context("编不出过滤规则（这是 spike 自己的设置，不是用户输入）")?;
    let started = Instant::now();
    let outcome = project_main_list(&bytes, &filter, true, None).context("投影评论列表失败")?;
    let project_ms = elapsed_ms(started);

    let entities = match outcome {
        MainListOutcome::Page(page) => page.replies,
        MainListOutcome::EmptyPage { .. } => {
            anyhow::bail!("整页被过滤光了——合成数据撞上了默认过滤规则，这是 spike 的 bug")
        }
    };

    let settings = ReplyRichTextSettings {
        enable_word_re: true,
        video_duration_ms: Some(video_duration_ms),
    };
    let started = Instant::now();
    let rows: Vec<Row> = entities
        .into_iter()
        .map(|entity| {
            let segments = segment(&input_of(&entity), &settings);
            Row { entity, segments }
        })
        .collect();
    let segment_ms = elapsed_ms(started);

    Ok(Built {
        rows,
        synth_ms,
        project_ms,
        segment_ms,
        encoded_bytes: bytes.len(),
    })
}

/// 实体 → 分词输入。基线在每次 widget build 上做同样的事。
#[must_use]
pub fn input_of(entity: &ReplyEntity) -> ReplyRichTextInput {
    ReplyRichTextInput {
        message: entity.content.message.clone(),
        emotes: entity.content.emotes.clone(),
        topics: entity.content.topics.clone(),
        at_name_to_mid: entity.content.at_name_to_mid.clone(),
        urls: entity.content.urls.clone(),
        vote: entity.content.vote.clone(),
        note: entity.content.note.clone(),
        is_note: entity.control.is_note,
        is_note_v2: entity.control.is_note_v2,
    }
}

fn elapsed_ms(since: Instant) -> f32 {
    since.elapsed().as_secs_f32() * 1000.0
}

fn reply(index: usize) -> pb::ReplyInfo {
    let id = 100_000 + index as i64;
    let mid = 3000 + (index as i64 % 997);
    let shape = index % SHAPES;
    let content = content_for(shape, index);
    pb::ReplyInfo {
        id: Some(id),
        oid: Some(2001),
        r#type: Some(1),
        mid: Some(mid),
        root: Some(0),
        parent: Some(0),
        dialog: Some(0),
        // 时间按下标往前推，列表里就有一串不同的「x 分钟前」。
        ctime: Some(1_763_000_000 - index as i64 * 137),
        like: Some((index as i64 * 7919) % 5000),
        count: Some((index as i64 * 13) % 48),
        content: Some(content),
        member: Some(pb::Member {
            mid: Some(mid),
            name: Some(nickname(index)),
            face: Some("https://example.invalid/face.jpg".to_owned()),
            level: Some((index as i64 % 6) + 1),
            vip_status: Some(i32::from(index.is_multiple_of(5)).into()),
            vip_type: Some(2),
            fans_medal_name: if index.is_multiple_of(4) {
                Some("牌子".to_owned())
            } else {
                None
            },
            fans_medal_level: Some(20),
            ..pb::Member::default()
        }),
        reply_control: Some(pb::ReplyControl {
            location: Some(format!("IP属地：{}", province(index))),
            ..pb::ReplyControl::default()
        }),
        ..pb::ReplyInfo::default()
    }
}

fn content_for(shape: usize, index: usize) -> pb::Content {
    let mut content = pb::Content::default();
    let message = match shape {
        0 => "前排".to_owned(),
        1 => {
            content.emotes = emotes();
            content.at_name_to_mid = [(nickname(index + 1), 3000 + (index as i64 + 1) % 997)]
                .into_iter()
                .collect();
            content.members = [(
                nickname(index + 1),
                pb::Member {
                    mid: Some(3000 + (index as i64 + 1) % 997),
                    name: Some(nickname(index + 1)),
                    ..pb::Member::default()
                },
            )]
            .into_iter()
            .collect();
            format!(
                "[doge] @{} 这段讲得是真的好，[妙啊] 建议反复看三遍",
                nickname(index + 1)
            )
        }
        2 => LONG_TEXT.to_owned(),
        3 => {
            let how_many = index % 4 + 1;
            content.pictures = (0..how_many)
                // 现在恰好把字段写全了，`..default()` 于是"没有效果"。
                // 仍然留着：`pb::Picture` 是生成的，上游加字段时这里应当
                // 自动取默认值，而不是让 fixture 构造编译不过。
                .map(|n| {
                    #[allow(clippy::needless_update)]
                    pb::Picture {
                        img_src: Some(format!("https://example.invalid/p{n}.jpg")),
                        img_width: Some(1080.0),
                        img_height: Some(720.0),
                        img_size: Some(256.0),
                        ..pb::Picture::default()
                    }
                })
                .collect();
            "随手拍了几张，顺便问问这是在哪个站拍的".to_owned()
        }
        4 => {
            content.urls = [(
                "av1".to_owned(),
                pb::Url {
                    title: Some("某稿件".to_owned()),
                    prefix_icon: Some("https://example.invalid/icon.png".to_owned()),
                    app_url_schema: Some("bilibili://video/1".to_owned()),
                    click_report: Some(r#"{"aid":1}"#.to_owned()),
                    ..pb::Url::default()
                },
            )]
            .into_iter()
            .collect();
            content.topics = [("杂谈".to_owned(), pb::Topic::default())]
                .into_iter()
                .collect();
            "03:21 这里的转场和 av1 里那段一模一样 #杂谈# 有人考据过吗".to_owned()
        }
        5 => {
            content.vote = Some(pb::Vote {
                id: Some(9001),
                title: Some("你站哪边".to_owned()),
            });
            "投票投起来 {vote:9001} 我先站第一个".to_owned()
        }
        6 => "笑死 😂 这波操作 I can't believe it 🎉 BGM 是 Lost River 吧".to_owned(),
        _ => {
            content.at_name_to_mid = [(nickname(index.wrapping_sub(1)), 3002)]
                .into_iter()
                .collect();
            format!(
                "回复 @{}：确实，第二遍才看懂",
                nickname(index.wrapping_sub(1))
            )
        }
    };
    content.message = Some(message);
    content
}

fn emotes() -> BTreeMap<String, pb::Emote> {
    [
        ("[doge]", "doge"),
        ("[妙啊]", "miao"),
        ("[OK]", "ok"),
        ("[笑哭]", "cry"),
    ]
    .into_iter()
    .map(|(placeholder, slug)| {
        (
            placeholder.to_owned(),
            pb::Emote {
                size: Some(1),
                url: Some(format!("https://example.invalid/{slug}.png")),
                webp_url: Some(format!("https://example.invalid/{slug}.webp")),
                ..pb::Emote::default()
            },
        )
    })
    .collect()
}

/// 昵称全是合成的。真实昵称属于要脱敏的那一类（AGENTS.md §3 的 golden
/// fixture 规矩），而这里连 fixture 都不是，直接编。
fn nickname(index: usize) -> String {
    const STEMS: [&str; 12] = [
        "长江七号",
        "不想起名字",
        "切片工",
        "一只鸽子",
        "深夜放毒",
        "考据党",
        "路过的",
        "前端苦工",
        "摸鱼日常",
        "字幕组打杂",
        "只看不评",
        "三连发射器",
    ];
    format!("{}{}", STEMS[index % STEMS.len()], index % 1000)
}

fn province(index: usize) -> &'static str {
    const P: [&str; 8] = [
        "北京", "上海", "广东", "浙江", "四川", "湖北", "江苏", "陕西",
    ];
    P[index % P.len()]
}

const LONG_TEXT: &str = "这一段其实在第一期里就埋过伏笔了，当时弹幕里还有人说是巧合。\
现在回头看，分镜的节奏、BGM 进来的时机、连那个一闪而过的路牌，都是对得上的。\
up 主在简介里写的那句话也有别的意思：不是说这期做得久，而是说素材是三年前就拍好的。\
所以才会出现冬天的外景接夏天的室内这种穿帮，其实不是穿帮，是故意留的线索。";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shape_survives_the_production_pipeline() {
        // 合成数据一旦撞上默认过滤规则（带货词、只含 @ 之类），整页会被
        // 删光，列表场景就变成在量一个空模型。
        let built = build(SHAPES * 3, 600_000).expect("管线应当跑通");
        assert_eq!(built.rows.len(), SHAPES * 3);
        assert!(built.rows.iter().all(|r| !r.segments.is_empty()));
    }

    #[test]
    fn the_decorated_shapes_really_carry_decorations() {
        let built = build(SHAPES, 600_000).expect("管线应当跑通");
        let kinds: Vec<_> = built
            .rows
            .iter()
            .flat_map(|r| r.segments.iter().map(|s| s.kind))
            .collect();
        use bili_domain::reply_richtext::ReplySegmentKind as K;
        for expected in [
            K::Emote,
            K::Mention,
            K::Topic,
            K::Link,
            K::Timestamp,
            K::Vote,
        ] {
            assert!(
                kinds.contains(&expected),
                "八种形状里应当至少有一条带 {expected:?}，否则富文本场景测不到它"
            );
        }
        assert!(
            built
                .rows
                .iter()
                .any(|r| !r.entity.content.pictures.is_empty())
        );
    }
}
