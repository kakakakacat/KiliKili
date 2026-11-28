//! listener 音频的 typed 导出。宿主只注入 gRPC 身份和播放偏好。

use bili_api::{audio, http::RequestContext, transport_reqwest::ReqwestTransport};
use bili_domain::{
    audio as domain,
    error::{DomainError, ErrorKind},
};

use crate::api::{grpc::GrpcContext, init, play_url::Volume};

#[derive(Debug, Clone, Copy)]
pub enum AudioPlaylistSource {
    Default,
    MemSpace,
    AudioCollection,
    AudioCard,
    UserFavourite,
    UpArchive,
    AudioCache,
    PickCard,
    MediaList,
}
impl AudioPlaylistSource {
    fn code(self) -> i32 {
        self as i32
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AudioListOrder {
    NoOrder,
    OrderNormal,
    OrderReverse,
    OrderRandom,
}
impl AudioListOrder {
    fn code(self) -> i32 {
        self as i32
    }
}

#[derive(Debug, Clone)]
pub struct AudioPart {
    pub oid: i64,
    pub sub_id: i64,
    pub title: String,
}

#[derive(Debug, Clone)]
pub struct AudioItem {
    pub oid: i64,
    pub sub_ids: Vec<i64>,
    pub item_type: i32,
    pub title: String,
    pub cover: String,
    pub desc: String,
    pub has_desc: bool,
    pub duration: i64,
    pub publish: i64,
    pub displayed_oid: String,
    pub copyright: i32,
    pub parts: Vec<AudioPart>,
    pub owner_mid: i64,
    pub owner_name: String,
    pub owner_avatar: String,
    pub has_owner_name: bool,
    pub has_owner_avatar: bool,
    pub like: i32,
    pub coin: i32,
    pub favourite: i32,
    pub reply: i32,
    pub share: i32,
    pub view: i32,
    pub has_like: bool,
    pub has_coin: bool,
    pub has_fav: bool,
    pub associated_oid: Option<i64>,
    pub associated_sub_ids: Vec<i64>,
}

impl From<domain::AudioDetail> for AudioItem {
    fn from(d: domain::AudioDetail) -> Self {
        Self {
            oid: d.item.oid,
            sub_ids: d.item.sub_id,
            item_type: d.item.item_type,
            title: d.arc.title,
            cover: d.arc.cover,
            desc: d.arc.desc,
            has_desc: d.has_desc,
            duration: d.arc.duration,
            publish: d.arc.publish,
            displayed_oid: d.arc.displayed_oid,
            copyright: d.arc.copyright,
            parts: d
                .parts
                .into_iter()
                .map(|p| AudioPart {
                    oid: p.oid,
                    sub_id: p.sub_id,
                    title: p.title,
                })
                .collect(),
            owner_mid: d.owner.mid,
            owner_name: d.owner.name,
            owner_avatar: d.owner.avatar,
            has_owner_name: d.owner.has_name,
            has_owner_avatar: d.owner.has_avatar,
            like: d.stat.like,
            coin: d.stat.coin,
            favourite: d.stat.favourite,
            reply: d.stat.reply,
            share: d.stat.share,
            view: d.stat.view,
            has_like: d.stat.has_like,
            has_coin: d.stat.has_coin,
            has_fav: d.stat.has_fav,
            associated_oid: d.has_associated_oid.then_some(d.associated_item.oid),
            associated_sub_ids: d.associated_item.sub_id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioPlaylist {
    pub reach_start: bool,
    pub reach_end: bool,
    pub prev: String,
    pub next: String,
    pub items: Vec<AudioItem>,
}
impl From<domain::AudioPlaylist> for AudioPlaylist {
    fn from(p: domain::AudioPlaylist) -> Self {
        Self {
            reach_start: p.reach_start,
            reach_end: p.reach_end,
            prev: p.prev,
            next: p.next,
            items: p.list.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioPlayback {
    pub url: Option<String>,
    pub volume: Option<Volume>,
}
impl From<domain::AudioPlayback> for AudioPlayback {
    fn from(p: domain::AudioPlayback) -> Self {
        Self {
            url: p.url,
            volume: p.volume.map(|v| Volume {
                measured_i: v.measured_i,
                measured_lra: v.measured_lra,
                measured_tp: v.measured_tp,
                measured_threshold: v.measured_threshold,
                target_offset: v.target_offset,
                target_i: v.target_i,
                target_tp: v.target_tp,
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioWrite {
    pub message: String,
    pub coin_ok: bool,
}
impl From<domain::AudioWriteReply> for AudioWrite {
    fn from(w: domain::AudioWriteReply) -> Self {
        Self {
            message: w.message,
            coin_ok: w.coin_ok,
        }
    }
}

macro_rules! result_type {
    ($name:ident, $data:ty) => {
        #[derive(Debug, Clone)]
        pub struct $name {
            pub data: Option<$data>,
            pub error_kind: Option<String>,
            pub error_detail: Option<String>,
            pub error_code: Option<i64>,
        }
        impl $name {
            fn from_result(r: Result<$data, DomainError>) -> Self {
                match r {
                    Ok(data) => Self {
                        data: Some(data),
                        error_kind: None,
                        error_detail: None,
                        error_code: None,
                    },
                    Err(e) => Self {
                        data: None,
                        error_kind: Some(e.kind().to_string()),
                        error_detail: Some(e.detail().to_owned()),
                        error_code: e.code(),
                    },
                }
            }
        }
    };
}
result_type!(AudioPlaylistResult, AudioPlaylist);
result_type!(AudioPlaybackResult, AudioPlayback);
result_type!(AudioWriteResult, AudioWrite);

fn item(oid: i64, sub_ids: Vec<i64>, item_type: i32) -> audio::AudioRequestItem {
    audio::AudioRequestItem {
        oid,
        sub_id: sub_ids,
        item_type,
    }
}

fn split(context: GrpcContext) -> (RequestContext, ReqwestTransport) {
    context.split()
}

fn flatten<T>(
    joined: Result<Result<T, DomainError>, tokio::task::JoinError>,
) -> Result<T, DomainError> {
    joined.unwrap_or_else(|_| Err(DomainError::new(ErrorKind::Protocol, "音频工作任务未完成")))
}

#[derive(Debug, Clone)]
pub struct AudioPlaylistQuery {
    pub id: i64,
    pub anchor_oid: Option<i64>,
    pub anchor_sub_ids: Vec<i64>,
    pub anchor_item_type: Option<i32>,
    pub source: Option<AudioPlaylistSource>,
    pub extra_id: Option<i64>,
    pub next: Option<String>,
    pub order: AudioListOrder,
}

pub async fn fetch_audio_playlist(
    context: GrpcContext,
    query: AudioPlaylistQuery,
) -> AudioPlaylistResult {
    let (ctx, transport) = split(context);
    let anchor = query.anchor_oid.map(|oid| {
        item(
            oid,
            query.anchor_sub_ids,
            query.anchor_item_type.unwrap_or_default(),
        )
    });
    let request = audio::AudioPlaylistRequest {
        id: query.id,
        anchor,
        source: query.source.map(AudioPlaylistSource::code),
        extra_id: query.extra_id,
        next: query.next,
        order: query.order.code(),
    };
    let r = flatten(
        init::runtime()
            .spawn(async move { audio::playlist(&transport, &ctx, request).await })
            .await,
    );
    AudioPlaylistResult::from_result(r.map(Into::into))
}

pub async fn fetch_audio_play_url(
    context: GrpcContext,
    oid: i64,
    sub_ids: Vec<i64>,
    item_type: i32,
    preferred_quality: i64,
    cdn_service: String,
    disable_audio_cdn: bool,
) -> AudioPlaybackResult {
    let (ctx, transport) = split(context);
    let r = flatten(
        init::runtime()
            .spawn(async move {
                audio::play_url(
                    &transport,
                    &ctx,
                    item(oid, sub_ids, item_type),
                    preferred_quality,
                    &cdn_service,
                    disable_audio_cdn,
                )
                .await
            })
            .await,
    );
    AudioPlaybackResult::from_result(r.map(Into::into))
}

pub async fn audio_thumb_up(
    context: GrpcContext,
    oid: i64,
    sub_ids: Vec<i64>,
    item_type: i32,
    like: bool,
) -> AudioWriteResult {
    let (ctx, transport) = split(context);
    let r = flatten(
        init::runtime()
            .spawn(async move {
                audio::thumb_up(&transport, &ctx, item(oid, sub_ids, item_type), like).await
            })
            .await,
    );
    AudioWriteResult::from_result(r.map(Into::into))
}

pub async fn audio_triple_like(
    context: GrpcContext,
    oid: i64,
    sub_ids: Vec<i64>,
    item_type: i32,
) -> AudioWriteResult {
    let (ctx, transport) = split(context);
    let r = flatten(
        init::runtime()
            .spawn(async move {
                audio::triple_like(&transport, &ctx, item(oid, sub_ids, item_type)).await
            })
            .await,
    );
    AudioWriteResult::from_result(r.map(Into::into))
}

pub async fn audio_coin_add(
    context: GrpcContext,
    oid: i64,
    sub_ids: Vec<i64>,
    item_type: i32,
    num: i32,
    thumb_up: bool,
) -> AudioWriteResult {
    let (ctx, transport) = split(context);
    let r = flatten(
        init::runtime()
            .spawn(async move {
                audio::coin_add(
                    &transport,
                    &ctx,
                    item(oid, sub_ids, item_type),
                    num,
                    thumb_up,
                )
                .await
            })
            .await,
    );
    AudioWriteResult::from_result(r.map(Into::into))
}
