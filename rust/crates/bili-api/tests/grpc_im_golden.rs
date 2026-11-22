//! IM gRPC 响应 → 领域实体的投影对照（S12 续单 / issue #63）。
//!
//! ## 这不是抓包样本
//!
//! 与 `reply_grpc_golden.rs` 同理，而且更极端：私信的响应里每条会话都带
//! 昵称、头像地址、最后一条消息的摘要和对方的 mid，脱敏之后剩不下任何
//! 可证明的东西。所以下面这份消息是**手工构造**的，写在代码里，让下一个
//! 人一眼看出每个字段为什么长这样。
//!
//! 因此本测试证明的是**投影规则**，不是「线上响应能解开」。后者按
//! AGENTS.md 留到 Phase 2 的真机回归（见 `docs/qa/RELEASE-VALIDATION.md`
//! 的 M33 / M34）。
//!
//! ## 它盯住的三件事
//!
//! 1. **头像按图层位置取**：第 0 层主图、第 1 层挂件、最后一层认证标。
//!    服务端没有任何字段标注哪一层是什么，位置就是语义。
//! 2. **折叠入口的会话类型 → 二级页类型**。两个枚举的数字几乎每行都不
//!    同；写这段时真的猜错过，而猜错之后单测仍然自洽（见
//!    `grpc_im` 里那条 `session_type_numbers_are_not_page_type_numbers`）。
//! 3. **`vip_info` 是一段 JSON 字符串**，不是子消息。
//!
//! 重新生成：
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p bili-api --test grpc_im_golden
//! ```

#![allow(clippy::expect_used)]
// `..default()` 是刻意的：生成面按 `KEEP` 裁过，有些消息眼下恰好没有别
// 的字段了。去掉它，下次放宽 `KEEP` 时这份 fixture 会编不过，而它本来
// 只想说「只设下面这几个字段」。
#![allow(clippy::needless_update)]

use std::fs;
use std::path::Path;

use bili_domain::im::{msg_type, session_type};
use bili_grpc::bilibili::app::im::v1 as pb;
use bili_grpc::bilibili::dagw::component::avatar::common as avatar_common;
use bili_grpc::bilibili::dagw::component::avatar::v1 as avatar;
use bili_grpc::bilibili::im::interfaces::v1 as ipb;
use bili_grpc::bilibili::im::r#type as tpb;
use prost::Message;
use serde_json::Value;

fn remote_layer(url: &str) -> avatar::Layer {
    avatar::Layer {
        resource: Some(avatar::BasicLayerResource {
            payload: Some(avatar::basic_layer_resource::Payload::ResImage(
                avatar::ResImage {
                    image_src: Some(avatar_common::ResourceSource {
                        res: Some(avatar_common::resource_source::Res::Remote(
                            avatar_common::RemoteRes {
                                url: Some(url.to_owned()),
                            },
                        )),
                        ..avatar_common::ResourceSource::default()
                    }),
                },
            )),
            ..avatar::BasicLayerResource::default()
        }),
        ..avatar::Layer::default()
    }
}

/// 认证标那一层：没有 url，装的是 `LocalRes` 的枚举值。
fn local_layer(value: i32) -> avatar::Layer {
    avatar::Layer {
        resource: Some(avatar::BasicLayerResource {
            payload: Some(avatar::basic_layer_resource::Payload::ResImage(
                avatar::ResImage {
                    image_src: Some(avatar_common::ResourceSource {
                        res: Some(avatar_common::resource_source::Res::LocalValue(value)),
                        ..avatar_common::ResourceSource::default()
                    }),
                },
            )),
            ..avatar::BasicLayerResource::default()
        }),
        ..avatar::Layer::default()
    }
}

/// 一条私聊会话：三层头像（主图 + 挂件 + 机构认证标）、年度会员、
/// 描边小标签、直播中、已置顶、有未读数字。
fn private_session() -> pb::Session {
    pb::Session {
        id: Some(pb::SessionId {
            id: Some(pb::session_id::Id::PrivateId(pb::PrivateId {
                talker_uid: Some(100_001),
            })),
        }),
        session_info: Some(pb::SessionInfo {
            session_name: Some("示例年度会员".to_owned()),
            avatar: Some(avatar::AvatarItem {
                fallback_layers: Some(avatar::LayerGroup {
                    layers: vec![
                        remote_layer("https://example.invalid/face.jpg"),
                        remote_layer("https://example.invalid/pendant.png"),
                        // 4 = LOCAL_RES_ICON_ENTERPRISE_VERIFY
                        local_layer(4),
                    ],
                    ..avatar::LayerGroup::default()
                }),
                mid: Some(100_001),
            }),
            // 协议把它塞成一段 JSON 字符串而不是子消息。
            vip_info: Some(r#"{"status":1,"type":2,"due_date":0}"#.to_owned()),
            user_label: Some(pb::UserLabel {
                style: Some(pb::UserLabelStyle {
                    style: Some(pb::user_label_style::Style::BorderedLabel(
                        pb::BorderedLabel {
                            text: Some("已关注".to_owned()),
                            ..pb::BorderedLabel::default()
                        },
                    )),
                }),
                ..pb::UserLabel::default()
            }),
            is_live: Some(true),
            ..pb::SessionInfo::default()
        }),
        unread: Some(pb::Unread {
            style: Some(pb::UnreadStyle::Number as i32),
            number: Some(7),
            ..pb::Unread::default()
        }),
        msg_summary: Some(pb::MsgSummary {
            raw_msg: Some("最后一条消息的摘要".to_owned()),
            ..pb::MsgSummary::default()
        }),
        // 微秒，不是毫秒也不是秒。
        timestamp: Some(1_731_000_000_000_000),
        is_pinned: Some(true),
        is_muted: Some(false),
        ..pb::Session::default()
    }
}

/// 一条折叠入口：AI 折叠（会话类型 9）应当映射到 AI 页（页面类型 7）。
/// 这两个数字不同，正是整张映射表存在的理由。
fn fold_session() -> pb::Session {
    pb::Session {
        id: Some(pb::SessionId {
            id: Some(pb::session_id::Id::FoldId(pb::FoldId {
                r#type: Some(session_type::AI_FOLD),
                ..pb::FoldId::default()
            })),
        }),
        session_info: Some(pb::SessionInfo {
            session_name: Some("AI 助手".to_owned()),
            // 只有一层：那一层既是「第 0 层」也是「最后一层」，于是它是
            // 主图而**不该**被当成认证标。
            avatar: Some(avatar::AvatarItem {
                fallback_layers: Some(avatar::LayerGroup {
                    layers: vec![remote_layer("https://example.invalid/ai.png")],
                    ..avatar::LayerGroup::default()
                }),
                ..avatar::AvatarItem::default()
            }),
            ..pb::SessionInfo::default()
        }),
        unread: Some(pb::Unread {
            style: Some(pb::UnreadStyle::Dot as i32),
            ..pb::Unread::default()
        }),
        ..pb::Session::default()
    }
}

/// 一条没有映射的折叠入口：会话类型是「私聊」，不该出现在折叠位上。
/// 界面此时 toast 出枚举名而不是猜一个页面。
fn unmapped_fold_session() -> pb::Session {
    pb::Session {
        id: Some(pb::SessionId {
            id: Some(pb::session_id::Id::FoldId(pb::FoldId {
                r#type: Some(session_type::PRIVATE),
                ..pb::FoldId::default()
            })),
        }),
        ..pb::Session::default()
    }
}

fn session_main_reply() -> pb::SessionMainReply {
    pb::SessionMainReply {
        pagination_params: Some(pb::PaginationParams {
            offsets: [(
                1,
                pb::Offset {
                    normal_offset: Some(4242),
                    top_offset: Some(7),
                },
            )]
            .into_iter()
            .collect(),
            has_more: Some(true),
        }),
        sessions: vec![private_session(), fold_session(), unmapped_fold_session()],
        three_dot_items: vec![
            pb::ThreeDotItem {
                title: Some("一键已读".to_owned()),
                r#type: Some(pb::ThreeDotItemType::ReadAll as i32),
                has_red_dot: Some(false),
                ..pb::ThreeDotItem::default()
            },
            pb::ThreeDotItem {
                title: Some("上游新加的一种".to_owned()),
                // 界面没有分支的类型应当收敛成 Other。
                r#type: Some(99),
                ..pb::ThreeDotItem::default()
            },
        ],
        outside_item: vec![pb::ThreeDotItem {
            title: Some("通讯录".to_owned()),
            r#type: Some(pb::ThreeDotItemType::Contacts as i32),
            ..pb::ThreeDotItem::default()
        }],
        ..pb::SessionMainReply::default()
    }
}

/// 一页消息：文字、图片、撤回、协议里没名字的 16、以及一种界面不认的。
fn session_msg_reply() -> ipb::RspSessionMsg {
    fn msg(seqno: i64, msg_type: i32, content: &str) -> tpb::Msg {
        tpb::Msg {
            sender_uid: Some(100_001),
            receiver_id: Some(100_002),
            msg_type: Some(msg_type),
            content: Some(content.to_owned()),
            msg_seqno: Some(seqno),
            timestamp: Some(1_731_000_000),
            msg_key: Some(seqno * 10),
            msg_status: Some(0),
            msg_source: Some(0),
            ..tpb::Msg::default()
        }
    }
    ipb::RspSessionMsg {
        messages: vec![
            msg(1, msg_type::TEXT, r#"{"content":"你好"}"#),
            msg(
                2,
                msg_type::PIC,
                r#"{"url":"https://example.invalid/p.jpg"}"#,
            ),
            // 自定义表情与图片共用一种画法。
            msg(
                3,
                msg_type::CUSTOM_FACE,
                r#"{"url":"https://example.invalid/e.png"}"#,
            ),
            msg(4, msg_type::DRAW_BACK, "40"),
            msg(5, msg_type::UNNAMED_16, r#"{"title":"没有名字的 16"}"#),
            msg(6, 12345, r#"{"x":1}"#),
        ],
        has_more: Some(1),
        e_infos: vec![
            ipb::EmotionInfo {
                text: Some("[doge]".to_owned()),
                url: Some("https://example.invalid/doge.png".to_owned()),
                size: Some(1),
                gif_url: Some(String::new()),
            },
            // 空 text 的那条应当被丢掉。
            ipb::EmotionInfo {
                text: Some(String::new()),
                url: Some("https://example.invalid/none.png".to_owned()),
                ..ipb::EmotionInfo::default()
            },
        ],
        ..ipb::RspSessionMsg::default()
    }
}

/// 一页设置：开关、选择、带子设置的跳转、带弹窗选择的跳转。
fn settings_reply() -> pb::GetImSettingsReply {
    pb::GetImSettingsReply {
        page_title: Some("私信设置".to_owned()),
        settings: [
            (
                1,
                pb::Setting {
                    content: Some(pb::setting::Content::Switch(pb::SettingSwitch {
                        switch_on: Some(true),
                        title: Some("接收陌生人私信".to_owned()),
                        subtitle: Some("关掉之后只有互关的人能私信你".to_owned()),
                    })),
                },
            ),
            (
                2,
                pb::Setting {
                    content: Some(pb::setting::Content::Select(pb::SettingSelect {
                        item: vec![
                            pb::SelectItem {
                                item_type: Some(10),
                                text: Some("所有人".to_owned()),
                                selected: Some(false),
                            },
                            pb::SelectItem {
                                item_type: Some(20),
                                text: Some("仅关注".to_owned()),
                                selected: Some(true),
                            },
                        ],
                    })),
                },
            ),
            (
                3,
                pb::Setting {
                    content: Some(pb::setting::Content::Redirect(pb::SettingRedirect {
                        title: Some("消息提醒".to_owned()),
                        // trailing 文字要从子设置里算出来：第一个选中的
                        // 选项优先于打开着的开关。
                        selected_summary: Some("不该用到这个".to_owned()),
                        content: Some(pb::setting_redirect::Content::SettingPage(
                            pb::Redirect2SettingPage {
                                parent_setting_type: Some(4),
                                sub_settings: [
                                    (
                                        1,
                                        pb::Setting {
                                            content: Some(pb::setting::Content::Select(
                                                pb::SettingSelect {
                                                    item: vec![pb::SelectItem {
                                                        item_type: Some(1),
                                                        text: Some("开启".to_owned()),
                                                        selected: Some(true),
                                                    }],
                                                },
                                            )),
                                        },
                                    ),
                                    (
                                        2,
                                        pb::Setting {
                                            content: Some(pb::setting::Content::Switch(
                                                pb::SettingSwitch {
                                                    switch_on: Some(true),
                                                    title: Some("这个不该被取到".to_owned()),
                                                    subtitle: None,
                                                },
                                            )),
                                        },
                                    ),
                                ]
                                .into_iter()
                                .collect(),
                                ..pb::Redirect2SettingPage::default()
                            },
                        )),
                        ..pb::SettingRedirect::default()
                    })),
                },
            ),
            (
                4,
                pb::Setting {
                    content: Some(pb::setting::Content::Redirect(pb::SettingRedirect {
                        title: Some("折叠设置".to_owned()),
                        selected_summary: Some("折叠".to_owned()),
                        content: Some(pb::setting_redirect::Content::WindowSelect(
                            pb::RedirectWindowSelect {
                                item: vec![
                                    pb::SelectItem {
                                        item_type: Some(1),
                                        text: Some("折叠".to_owned()),
                                        selected: Some(true),
                                    },
                                    pb::SelectItem {
                                        item_type: Some(2),
                                        text: Some("不折叠".to_owned()),
                                        selected: Some(false),
                                    },
                                ],
                                ..pb::RedirectWindowSelect::default()
                            },
                        )),
                        ..pb::SettingRedirect::default()
                    })),
                },
            ),
        ]
        .into_iter()
        .collect(),
    }
}

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/grpc_im")
        .join(name)
}

fn check(name: &str, got: Value) {
    let path = fixture(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        fs::create_dir_all(path.parent().expect("有父目录")).expect("可建目录");
        let pretty = serde_json::to_string_pretty(&got).expect("可序列化");
        fs::write(&path, format!("{pretty}\n")).expect("快照可写");
        return;
    }
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(&path).expect("缺少快照，用 UPDATE_GOLDEN=1 重新生成"),
    )
    .expect("快照必须是合法 JSON");
    assert_eq!(got, expected, "{name} 的投影结果与冻结快照不一致");
}

fn value<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("实体可序列化")
}

#[test]
fn session_main_projection_matches_the_frozen_snapshot() {
    let page = bili_api::grpc_im::project_session_main(&session_main_reply().encode_to_vec())
        .expect("应解析成功");
    check("session_main.rust.json", value(&page));
}

#[test]
fn session_messages_projection_matches_the_frozen_snapshot() {
    let page = bili_api::grpc_im::project_session_msgs(&session_msg_reply().encode_to_vec())
        .expect("应解析成功");
    check("session_msgs.rust.json", value(&page));
}

#[test]
fn settings_projection_matches_the_frozen_snapshot() {
    let page =
        bili_api::grpc_im::project_settings(&settings_reply().encode_to_vec()).expect("应解析成功");
    check("settings.rust.json", value(&page));
}

/// 头像那三层的取法，单独点名——它是整段投影里最脆的一处，而快照里
/// 它只是三个字符串，改坏了不一定有人注意到。
#[test]
fn avatar_layers_are_read_by_position() {
    let page = bili_api::grpc_im::project_session_main(&session_main_reply().encode_to_vec())
        .expect("应解析成功");
    let first = &page.sessions[0];
    assert_eq!(
        first.avatar_url.as_deref(),
        Some("https://example.invalid/face.jpg"),
        "第 0 层是主头像"
    );
    assert_eq!(
        first.pendant_url.as_deref(),
        Some("https://example.invalid/pendant.png"),
        "第 1 层是挂件"
    );
    assert_eq!(first.official_type, Some(1), "最后一层是认证标，4 → 机构");

    // 只有一层时，「最后一层」就是主图：它没有 local_value，于是认证标
    // 是 None。否则每个没有挂件的人都会凭空多出个认证标。
    let only_one = &page.sessions[1];
    assert!(only_one.avatar_url.is_some());
    assert_eq!(only_one.pendant_url, None);
    assert_eq!(only_one.official_type, None);
}

/// 会话类型与页面类型的数字不同，这条盯住那个差。
#[test]
fn fold_type_is_mapped_not_passed_through() {
    let page = bili_api::grpc_im::project_session_main(&session_main_reply().encode_to_vec())
        .expect("应解析成功");
    let fold = &page.sessions[1];
    assert_eq!(fold.target.session_type, Some(session_type::AI_FOLD));
    assert_eq!(fold.target.fold_page_type, Some(7), "AI 页是 7，不是 9");
    assert_ne!(
        fold.target.fold_page_type, fold.target.session_type,
        "映射表存在的全部理由就是这两个数字不一样"
    );

    let unmapped = &page.sessions[2];
    assert_eq!(unmapped.target.fold_page_type, None);
    assert_eq!(
        unmapped.target.type_name.as_deref(),
        Some("SESSION_TYPE_PRIVATE"),
        "没有对应页面时界面要 toast 这个名字"
    );
}

/// 撤回与不认识的类型都要能走到，且 `kind` 与原始 `msg_type` 都在。
#[test]
fn message_kinds_cover_draw_back_and_unknown() {
    let page = bili_api::grpc_im::project_session_msgs(&session_msg_reply().encode_to_vec())
        .expect("应解析成功");
    use bili_domain::im::MsgKind;
    let kinds: Vec<MsgKind> = page.messages.iter().map(|m| m.kind).collect();
    assert_eq!(
        kinds,
        vec![
            MsgKind::Text,
            MsgKind::Picture,
            // 自定义表情与图片共用一种画法。
            MsgKind::Picture,
            MsgKind::DrawBack,
            MsgKind::Unnamed16,
            MsgKind::Unsupported,
        ]
    );
    // 原始取值要留着：`Unsupported` 那一档靠它说清是哪一种。
    assert_eq!(page.messages[5].msg_type, 12345);
    // 空 text 的表情条目丢掉。
    assert_eq!(page.emotes.len(), 1);
    assert_eq!(page.emotes[0].gif_url, None, "空串归一成 None");
}

/// trailing 文字的三级回退：子设置里选中的选项优先于打开着的开关，
/// 两者都没有才用 `selected_summary`。
#[test]
fn redirect_trailing_text_follows_the_baseline_fallback() {
    let page =
        bili_api::grpc_im::project_settings(&settings_reply().encode_to_vec()).expect("应解析成功");
    let redirect = page
        .settings
        .iter()
        .find(|s| s.key == 3)
        .expect("有这一项")
        .redirect
        .as_ref()
        .expect("是跳转型");
    assert_eq!(
        redirect.trailing_text.as_deref(),
        Some("开启"),
        "选中的选项优先于打开着的开关，也优先于 selected_summary"
    );

    let window = page
        .settings
        .iter()
        .find(|s| s.key == 4)
        .expect("有这一项")
        .redirect
        .as_ref()
        .expect("是跳转型");
    assert_eq!(window.window_options.len(), 2);
    assert_eq!(window.window_selected_index, Some(0));
    assert_eq!(window.trailing_text.as_deref(), Some("折叠"));
}
