//! 评论 gRPC 响应 → 领域实体的投影对照（S2 续单 / issue #176）。
//!
//! ## 这不是抓包样本
//!
//! `fixtures/grpc/README.md` 里那两份是真实响应的脱敏投影。评论这条
//! 路径拿不到同样的东西：`MainList` 的匿名响应里，每条评论都带昵称、
//! 头像、正文与 IP 属地，脱敏之后剩下的就是下面这份**手工构造**的
//! 消息——与其提交一个「看起来像抓包、其实每个字段都是编的」二进制，
//! 不如把它写在代码里，让下一个人一眼看出每个字段为什么长这样。
//!
//! 因此本测试证明的是**投影规则**（哪个 pb 字段去了实体的哪里、回退链
//! 与阈值怎么算），不是「线上响应能解开」。后者按 AGENTS.md 留到
//! Phase 2 的真机回归。
//!
//! 期望值存成 JSON（`reply_grpc/main_list.rust.json`）而不是写在断言里：
//! 投影改了之后 diff 能直接看出动了哪个字段。比较的是解析后的 JSON 值而
//! 不是字符串——实体里的 map 是 `HashMap`，序列化顺序不稳定。

// 测试里 expect 是对的：前提不成立时就该崩掉并指出原因。
#![allow(clippy::expect_used)]
// `..default()` 在这里是刻意的：生成面按 `KEEP` 裁过，有些消息眼下恰好
// 没有别的字段了。去掉它，下次放宽 `KEEP` 时这份 fixture 会编不过，而它
// 本来只想说「只设下面这几个字段」。
#![allow(clippy::needless_update)]

use std::fs;
use std::path::Path;

use bili_api::grpc_reply::{MainListOutcome, ReplyMode, main_list_request};
use bili_domain::reply_filter::{ReplyFilter, ReplyFilterSettings};
use bili_grpc::bilibili::main::community::reply::v1 as pb;
use prost::Message;
use serde_json::Value;

/// 一条带全套装饰的根评论：表情、@、话题、链接、图片、投票、粉丝牌、
/// 装扮卡、IP 属地、翻译开关、卡片标签，外加两条楼中楼。
fn decorated_reply() -> pb::ReplyInfo {
    pb::ReplyInfo {
        id: Some(1001),
        oid: Some(2001),
        r#type: Some(1),
        mid: Some(3001),
        root: Some(0),
        parent: Some(0),
        dialog: Some(0),
        ctime: Some(1_700_000_000),
        like: Some(42),
        count: Some(7),
        content: Some(pb::Content {
            message: Some("[doge] @甲 #话题# av1 看这里".to_owned()),
            emotes: [(
                "[doge]".to_owned(),
                pb::Emote {
                    size: Some(1),
                    // 三档都给：投影要挑 webp。
                    url: Some("https://example.invalid/e.png".to_owned()),
                    gif_url: Some("https://example.invalid/e.gif".to_owned()),
                    webp_url: Some("https://example.invalid/e.webp".to_owned()),
                    jump_url: Some("https://example.invalid/jump".to_owned()),
                    ..pb::Emote::default()
                },
            )]
            .into_iter()
            .collect(),
            topics: [("话题".to_owned(), pb::Topic::default())]
                .into_iter()
                .collect(),
            at_name_to_mid: [("甲".to_owned(), 3002)].into_iter().collect(),
            urls: [(
                "av1".to_owned(),
                pb::Url {
                    title: Some("某稿件".to_owned()),
                    prefix_icon: Some("https://example.invalid/icon.png".to_owned()),
                    app_url_schema: Some("bilibili://video/1".to_owned()),
                    // 不是 `{"cvid` 开头，所以不是笔记。
                    click_report: Some(r#"{"aid":1}"#.to_owned()),
                    extra: Some(pb::url::Extra {
                        is_word_search: Some(true),
                        ..pb::url::Extra::default()
                    }),
                    ..pb::Url::default()
                },
            )]
            .into_iter()
            .collect(),
            vote: Some(pb::Vote {
                id: Some(9001),
                title: Some("投票标题".to_owned()),
            }),
            pictures: vec![pb::Picture {
                img_src: Some("https://example.invalid/p.jpg".to_owned()),
                img_width: Some(640.0),
                img_height: Some(480.0),
                img_size: Some(123.0),
                ..pb::Picture::default()
            }],
            members: [(
                "甲".to_owned(),
                pb::Member {
                    mid: Some(3002),
                    name: Some("甲".to_owned()),
                    ..pb::Member::default()
                },
            )]
            .into_iter()
            .collect(),
            ..pb::Content::default()
        }),
        member: Some(pb::Member {
            mid: Some(3001),
            name: Some("乙".to_owned()),
            face: Some("https://example.invalid/face.jpg".to_owned()),
            level: Some(6),
            // 0 是「个人认证」，投影后必须是 Some(0) 而不是 None。
            official_verify_type: Some(0),
            vip_type: Some(2),
            vip_status: Some(1),
            garb_pendant_image: Some("https://example.invalid/pendant.png".to_owned()),
            fans_medal_name: Some("牌子".to_owned()),
            fans_medal_level: Some(20),
            fans_medal_color: Some(0x00_55_55),
            fans_medal_color_name: Some(0xff_ff_ff),
            is_senior_member: Some(1),
            ..pb::Member::default()
        }),
        member_v2: Some(pb::MemberV2 {
            garb: Some(pb::member_v2::Garb {
                card_image: Some("https://example.invalid/card.png".to_owned()),
                card_number: Some("000123".to_owned()),
                fan_num_prefix: Some("NO.".to_owned()),
                card_fan_color: Some("#FFD700".to_owned()),
                ..pb::member_v2::Garb::default()
            }),
            ..pb::MemberV2::default()
        }),
        reply_control: Some(pb::ReplyControl {
            action: Some(1),
            up_reply: Some(true),
            is_up_top: Some(true),
            is_note: Some(true),
            is_note_v2: Some(true),
            location: Some("IP属地：上海".to_owned()),
            translation_switch: Some(pb::TranslationSwitch::ShowTranslation as i32),
            card_labels: vec![pb::ReplyCardLabel {
                text_content: Some("热评".to_owned()),
                ..pb::ReplyCardLabel::default()
            }],
            vote_option: Some(pb::reply_control::VoteOption {
                label_kind: Some(1),
                desc: Some("红方观点".to_owned()),
                ..pb::reply_control::VoteOption::default()
            }),
            ..pb::ReplyControl::default()
        }),
        parent_reply_member: Some(pb::ParentReplyMember {
            mid: Some(3002),
            name: Some("甲".to_owned()),
        }),
        replies: vec![
            nested_reply(1002, 3002, "楼中楼一"),
            // 这条会被拉黑规则删掉，根评论留着。
            nested_reply(1003, 4001, "楼中楼二"),
        ],
        ..pb::ReplyInfo::default()
    }
}

fn nested_reply(id: i64, mid: i64, message: &str) -> pb::ReplyInfo {
    pb::ReplyInfo {
        id: Some(id),
        oid: Some(2001),
        r#type: Some(1),
        mid: Some(mid),
        root: Some(1001),
        parent: Some(1001),
        ctime: Some(1_700_000_100),
        content: Some(pb::Content {
            message: Some(message.to_owned()),
            ..pb::Content::default()
        }),
        member: Some(pb::Member {
            mid: Some(mid),
            name: Some("丙".to_owned()),
            ..pb::Member::default()
        }),
        ..pb::ReplyInfo::default()
    }
}

/// 笔记评论：`click_report` 以 `{"cvid` 开头，并带一个 opus 跳转。
fn note_reply() -> pb::ReplyInfo {
    pb::ReplyInfo {
        id: Some(1004),
        oid: Some(2001),
        r#type: Some(1),
        mid: Some(3003),
        ctime: Some(1_700_000_200),
        content: Some(pb::Content {
            message: Some("cv1 笔记".to_owned()),
            urls: [(
                "cv1".to_owned(),
                pb::Url {
                    title: Some("某笔记".to_owned()),
                    click_report: Some(r#"{"cvid":1}"#.to_owned()),
                    ..pb::Url::default()
                },
            )]
            .into_iter()
            .collect(),
            rich_text: Some(pb::RichText {
                item: Some(pb::rich_text::Item::Opus(
                    bili_grpc::bilibili::app::dynamic::v2::OpusItem {
                        opus_id: Some(5001),
                        ..Default::default()
                    },
                )),
            }),
            ..pb::Content::default()
        }),
        member: Some(pb::Member {
            mid: Some(3003),
            name: Some("丁".to_owned()),
            ..pb::Member::default()
        }),
        ..pb::ReplyInfo::default()
    }
}

fn main_list() -> pb::MainListReply {
    pb::MainListReply {
        up_top: Some(pb::ReplyInfo {
            id: Some(1000),
            oid: Some(2001),
            r#type: Some(1),
            mid: Some(3001),
            ctime: Some(1_699_000_000),
            content: Some(pb::Content {
                message: Some("UP 置顶".to_owned()),
                ..pb::Content::default()
            }),
            member: Some(pb::Member {
                mid: Some(3001),
                name: Some("乙".to_owned()),
                ..pb::Member::default()
            }),
            ..pb::ReplyInfo::default()
        }),
        replies: vec![
            decorated_reply(),
            note_reply(),
            // 这条整条会被拉黑规则删掉。
            nested_reply(1005, 4001, "被拉黑的人的评论"),
        ],
        subject_control: Some(pb::SubjectControl {
            up_mid: Some(3001),
            count: Some(123),
            title: Some("全部评论".to_owned()),
            show_title: Some(true),
            input_disable: Some(false),
            root_text: Some("仅粉丝可发".to_owned()),
            ..pb::SubjectControl::default()
        }),
        vote_card: Some(pb::VoteCard {
            vote_id: Some(9001),
            title: Some("投票标题".to_owned()),
            count: Some(100),
            ..pb::VoteCard::default()
        }),
        mixed_cards: vec![pb::MixedCard {
            r#type: Some(pb::mixed_card::Type::Fold as i32),
            item: Some(pb::mixed_card::Item::Fold(pb::FoldCard {
                bottom_text: Some("查看折叠评论".to_owned()),
                fold_pagination: Some(bili_grpc::bilibili::pagination::FeedPagination {
                    offset: Some("fold-cursor".to_owned()),
                    ..Default::default()
                }),
            })),
            ..pb::MixedCard::default()
        }],
        pagination_reply: Some(bili_grpc::bilibili::pagination::FeedPaginationReply {
            next_offset: Some("next-cursor".to_owned()),
            ..Default::default()
        }),
        cursor: Some(pb::CursorReply {
            is_end: Some(false),
            ..pb::CursorReply::default()
        }),
        ..pb::MainListReply::default()
    }
}

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/reply_grpc")
        .join(name)
}

/// 把 pb 的一页根评论按 `fetch_main_list` 的顺序过一遍：过滤 → 投影。
///
/// 网络那一段不参与（它由 `grpc.rs` 的解帧与 `grpc_fetch.rs` 的桩覆盖），
/// 这里只跑响应落地之后的部分。
fn project_page(filter: &ReplyFilter) -> Value {
    let bytes = main_list().encode_to_vec();
    let outcome = bili_api::grpc_reply::project_main_list(&bytes, filter, true, None)
        .expect("应能解析并投影");
    let MainListOutcome::Page(page) = outcome else {
        panic!("这一页不该被过滤光");
    };
    serde_json::to_value(&page).expect("实体可序列化")
}

fn filter() -> ReplyFilter {
    ReplyFilter::compile(&ReplyFilterSettings {
        // 4001 是上面那两条「被拉黑的人」的 mid。
        blocked_mids: vec![4001],
        ..ReplyFilterSettings::default()
    })
    .expect("设置可编译")
}

#[test]
fn the_projection_matches_the_golden_file() {
    let actual = project_page(&filter());
    let path = fixture("main_list.rust.json");
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
    )
    .expect("golden 是 JSON");
    assert_eq!(
        actual, expected,
        "投影结果与 golden 不一致。确认改动是有意的之后，用 \
         `cargo test -p bili-api --test reply_grpc_golden -- --ignored update_golden` 重写"
    );
}

/// 重写 golden。只在确认投影改动是有意的之后手动跑。
#[test]
#[ignore = "手动重写 golden"]
fn update_golden() {
    let actual = project_page(&filter());
    fs::write(
        fixture("main_list.rust.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&actual).expect("可序列化")
        ),
    )
    .expect("可写入");
}

/// 请求面不随投影一起漂：字段号与取值由 `MainListReq` 的解码证明。
#[test]
fn the_request_carries_what_the_baseline_carried() {
    let decoded = pb::MainListReq::decode(
        main_list_request(2001, 1, Some(1001), ReplyMode::Hot, Some("c")).as_slice(),
    )
    .expect("应能解回来");
    assert_eq!(decoded.oid, Some(2001));
    assert_eq!(decoded.r#type, Some(1));
    assert_eq!(decoded.rpid, Some(1001));
    assert_eq!(decoded.mode, Some(pb::Mode::MainListHot as i32));
    assert_eq!(
        decoded.pagination.and_then(|p| p.offset).as_deref(),
        Some("c")
    );
    assert!(decoded.cursor.is_none(), "基线把 cursor 注释掉了，照抄");
}
