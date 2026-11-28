//! S16 音频投影对照（#203）。
//!
//! 来源：按迁移前 `lib/grpc/audio.dart` 使用的字段手工构造 protobuf 响应。
//! 匿名请求 listener PlayURL 返回 -101；没有可脱敏的真实响应，所以本
//! fixture 只约束字段投影与音质/CDN 规则，线上形状留 Phase 2 真机确认。

#![allow(clippy::expect_used)]

fn fixture(name: &str, bytes: &[u8]) -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audio");
    let path = root.join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(&root).expect("创建 fixture 目录");
        std::fs::write(&path, bytes).expect("写入 fixture");
    }
    let frozen = std::fs::read(path).expect("读取冻结 fixture");
    assert_eq!(frozen, bytes, "fixture 变化后须更新并重新做一次 parity");
    frozen
}

use bili_api::audio::{decode_play_url, decode_playlist};
use bili_api::{
    audio::{self, AudioPlaylistRequest, AudioRequestItem},
    grpc,
    http::{
        HttpBytesResponse, HttpGet, HttpPost, HttpResponse, HttpTransport, Idempotency,
        RequestContext, RetryPolicy,
    },
};
use bili_grpc::bilibili::{
    app::{listener::v1 as pb, playurl::v1::VolumeInfo},
    pagination::PaginationReply,
};
use prost::Message;
use std::{sync::Mutex, time::Duration};

#[derive(Default)]
struct Capture(Mutex<Vec<HttpPost>>);
impl HttpTransport for Capture {
    async fn get(&self, _: HttpGet) -> Result<HttpResponse, bili_domain::error::DomainError> {
        unreachable!("listener 只有 unary POST")
    }

    async fn post(&self, _: HttpPost) -> Result<HttpResponse, bili_domain::error::DomainError> {
        unreachable!("listener 只有二进制 unary POST")
    }

    async fn post_bytes(
        &self,
        request: HttpPost,
    ) -> Result<HttpBytesResponse, bili_domain::error::DomainError> {
        self.0.lock().expect("请求锁").push(request);
        Ok(HttpBytesResponse {
            status: 200,
            headers: vec![("grpc-status".into(), "0".into())],
            body: grpc::frame_request(&[]).expect("空响应帧"),
        })
    }
}

fn context() -> RequestContext {
    RequestContext {
        base_url: "https://example.invalid".into(),
        cookie: None,
        csrf: None,
        headers: vec![],
        timeout: Duration::from_secs(10),
    }
}

fn request_item() -> AudioRequestItem {
    AudioRequestItem {
        oid: 123,
        sub_id: vec![456],
        item_type: 1,
    }
}

#[tokio::test]
async fn writes_keep_protocol_values_and_retry_grade() {
    let capture = Capture::default();
    let ctx = context();
    audio::thumb_up(&capture, &ctx, request_item(), false)
        .await
        .expect("取消点赞");
    audio::triple_like(&capture, &ctx, request_item())
        .await
        .expect("三连");
    audio::coin_add(&capture, &ctx, request_item(), 2, true)
        .await
        .expect("投币");
    let requests = capture.0.lock().expect("请求锁");
    assert_eq!(requests.len(), 3);
    assert!(
        requests[0]
            .url
            .ends_with("/bilibili.app.listener.v1.Listener/ThumbUp")
    );
    assert!(
        requests[1]
            .url
            .ends_with("/bilibili.app.listener.v1.Listener/TripleLike")
    );
    assert!(
        requests[2]
            .url
            .ends_with("/bilibili.app.listener.v1.Listener/CoinAdd")
    );
    assert_eq!(
        requests[0].retry_policy,
        RetryPolicy::Write(Idempotency::Idempotent)
    );
    assert_eq!(
        requests[1].retry_policy,
        RetryPolicy::Write(Idempotency::Verifiable)
    );
    assert_eq!(
        requests[2].retry_policy,
        RetryPolicy::Write(Idempotency::Verifiable)
    );
    let body = match &requests[0].body {
        bili_api::http::PostBody::Bytes { data, .. } => data,
        _ => panic!("gRPC 必须发二进制帧"),
    };
    let decoded =
        pb::ThumbUpReq::decode(grpc::decode_frame(body).expect("解帧").as_slice()).expect("解请求");
    assert_eq!(decoded.item.expect("item").sub_id, [456]);
    assert_eq!(
        decoded.action,
        Some(pb::thumb_up_req::ThumbType::CancelLike as i32)
    );
    let coin_body = match &requests[2].body {
        bili_api::http::PostBody::Bytes { data, .. } => data,
        _ => panic!("gRPC 必须发二进制帧"),
    };
    let coin = pb::CoinAddReq::decode(grpc::decode_frame(coin_body).expect("解帧").as_slice())
        .expect("解请求");
    assert_eq!(coin.num, Some(2));
    assert_eq!(coin.thumb_up, Some(true));
}

#[tokio::test]
async fn reads_keep_player_args_and_pagination_shape() {
    let capture = Capture::default();
    let ctx = context();
    audio::play_url(&capture, &ctx, request_item(), 80, "baseUrl", false)
        .await
        .expect("取流请求");
    audio::playlist(
        &capture,
        &ctx,
        AudioPlaylistRequest {
            id: 789,
            anchor: Some(request_item()),
            source: Some(pb::PlaylistSource::AudioCard as i32),
            extra_id: Some(4),
            next: Some("cursor".into()),
            order: pb::ListOrder::OrderReverse as i32,
        },
    )
    .await
    .expect("列表请求");
    let requests = capture.0.lock().expect("请求锁");
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].retry_policy, RetryPolicy::Read);
    assert_eq!(requests[1].retry_policy, RetryPolicy::Read);
    assert!(
        requests[0]
            .url
            .ends_with("/bilibili.app.listener.v1.Listener/PlayURL")
    );
    assert!(
        requests[1]
            .url
            .ends_with("/bilibili.app.listener.v1.Listener/Playlist")
    );
    let body = |request: &HttpPost| match &request.body {
        bili_api::http::PostBody::Bytes { data, .. } => grpc::decode_frame(data).expect("解帧"),
        _ => panic!("gRPC 必须发二进制帧"),
    };
    let play = pb::PlayUrlReq::decode(body(&requests[0]).as_slice()).expect("解取流请求");
    let args = play.player_args.expect("PlayerArgs");
    assert_eq!(
        (args.qn, args.fnval, args.force_host, args.voice_balance),
        (Some(80), Some(4048), Some(2), Some(1))
    );
    assert_eq!(play.item.expect("PlayItem").sub_id, [456]);
    let playlist = pb::PlaylistReq::decode(body(&requests[1]).as_slice()).expect("解列表请求");
    assert_eq!(playlist.from, Some(pb::PlaylistSource::AudioCard as i32));
    assert_eq!(playlist.id, Some(789));
    assert_eq!(playlist.anchor.expect("anchor").oid, Some(123));
    assert_eq!(playlist.pagination.expect("pagination").page_size, Some(20));
    assert_eq!(
        playlist.sort_opt.expect("sort").order,
        Some(pb::ListOrder::OrderReverse as i32)
    );
}

#[test]
fn playlist_projects_only_consumed_fields() {
    let response = pb::PlaylistResp {
        reach_start: Some(false),
        reach_end: Some(true),
        pagination_reply: Some(PaginationReply {
            prev: Some("previous-page".into()),
            next: Some("next-page".into()),
        }),
        list: vec![pb::DetailItem {
            item: Some(pb::PlayItem {
                oid: Some(123),
                sub_id: vec![456],
                item_type: Some(1),
            }),
            arc: Some(pb::BkArchive {
                title: Some("脱敏标题".into()),
                cover: Some("https://example.invalid/cover.jpg".into()),
                desc: Some("脱敏简介".into()),
                duration: Some(100),
                publish: Some(1_700_000_000),
                displayed_oid: Some("BV1xxxxxxxxx".into()),
                copyright: Some(1),
            }),
            parts: vec![pb::BkArcPart {
                oid: Some(123),
                sub_id: Some(456),
                title: Some("第一段".into()),
            }],
            owner: Some(pb::Author {
                mid: Some(99),
                name: Some("脱敏昵称".into()),
                avatar: Some("https://example.invalid/avatar.jpg".into()),
            }),
            stat: Some(pb::BkStat {
                like: Some(5),
                coin: Some(2),
                favourite: Some(3),
                reply: Some(4),
                share: Some(6),
                view: Some(7),
                has_like: Some(true),
                has_coin: Some(false),
                has_fav: Some(true),
            }),
            associated_item: Some(pb::PlayItem {
                oid: Some(321),
                sub_id: vec![654],
                item_type: Some(1),
            }),
        }],
    };
    let bytes = fixture("playlist.raw.pb", &response.encode_to_vec());
    let page = decode_playlist(&bytes).expect("列表应解码");
    assert!(!page.reach_start);
    assert!(page.reach_end);
    assert_eq!(page.prev, "previous-page");
    assert_eq!(page.next, "next-page");
    let item = &page.list[0];
    assert_eq!(item.item.oid, 123);
    assert_eq!(item.item.sub_id, [456]);
    assert_eq!(item.arc.title, "脱敏标题");
    assert_eq!(item.arc.duration, 100);
    assert_eq!(item.owner.mid, 99);
    assert!(item.owner.has_name);
    assert_eq!(item.stat.like, 5);
    assert!(item.stat.has_like);
    assert_eq!(item.parts[0].sub_id, 456);
    assert_eq!(item.associated_item.oid, 321);
}

#[test]
fn play_url_uses_core_quality_and_cdn_rules() {
    let dash_item = |id, base: &str| pb::DashItem {
        id: Some(id),
        base_url: Some(base.into()),
        backup_url: vec![],
    };
    let response = pb::PlayUrlResp {
        player_info: [(
            0,
            pb::PlayInfo {
                volume: Some(VolumeInfo {
                    measured_i: Some(-13.0),
                    target_i: Some(-16.0),
                    ..Default::default()
                }),
                info: Some(pb::play_info::Info::PlayDash(pb::PlayDash {
                    audio: vec![
                        dash_item(302, "https://example.invalid/302"),
                        dash_item(64, "https://example.invalid/64"),
                        dash_item(132, "https://example.invalid/132"),
                    ],
                })),
            },
        )]
        .into(),
    };
    let bytes = fixture("play_url_dash.raw.pb", &response.encode_to_vec());
    let playback = decode_play_url(&bytes, 200, "baseUrl", false).expect("取流应解码");
    assert_eq!(playback.url.as_deref(), Some("https://example.invalid/132"));
    let volume = playback.volume.expect("应保留音量参数");
    assert_eq!(volume.measured_i, -13.0);
    assert_eq!(volume.target_i, -16.0);
}

#[test]
fn durl_uses_first_segment() {
    let response = pb::PlayUrlResp {
        player_info: [(
            0,
            pb::PlayInfo {
                volume: None,
                info: Some(pb::play_info::Info::PlayUrl(pb::PlayUrl {
                    durl: vec![
                        pb::ResponseUrl {
                            url: Some("https://example.invalid/first".into()),
                            backup_url: vec!["https://example.invalid/backup".into()],
                        },
                        pb::ResponseUrl {
                            url: Some("https://example.invalid/second".into()),
                            backup_url: vec![],
                        },
                    ],
                })),
            },
        )]
        .into(),
    };
    let bytes = fixture("play_url_durl.raw.pb", &response.encode_to_vec());
    let playback = decode_play_url(&bytes, 80, "baseUrl", false).expect("durl 应解码");
    assert_eq!(
        playback.url.as_deref(),
        Some("https://example.invalid/first")
    );
}

#[test]
fn player_info_uses_wire_insertion_order_not_sorted_key_order() {
    fn entry(key: u8, url: &str) -> Vec<u8> {
        let info = pb::PlayInfo {
            volume: None,
            info: Some(pb::play_info::Info::PlayUrl(pb::PlayUrl {
                durl: vec![pb::ResponseUrl {
                    url: Some(url.into()),
                    backup_url: vec![],
                }],
            })),
        }
        .encode_to_vec();
        assert!(info.len() < 128);
        let mut bytes = vec![
            0x22,
            (info.len() + 4) as u8,
            0x08,
            key,
            0x12,
            info.len() as u8,
        ];
        bytes.extend(info);
        bytes
    }
    let bytes = [entry(5, "first"), entry(1, "second")].concat();
    let playback = decode_play_url(&bytes, 80, "baseUrl", false).expect("取流应解码");
    assert_eq!(playback.url.as_deref(), Some("first"));
}
