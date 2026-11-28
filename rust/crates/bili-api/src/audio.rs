//! listener 音频的五条 gRPC。请求与响应都不把 protobuf 泄到宿主。
//!
//! 写操作按 ADR-013 定级：ThumbUp 是置为语义；TripleLike 同时投币、收藏，
//! CoinAdd 增加硬币，两者与视频投币同属 Verifiable，发前未查询故不自动重发。
//! gRPC 基线没有 csrf（ADR-013 决策七）。

use bili_core::{
    cdn::{CdnService, pick_cdn_url},
    media_select::find_target_media_index,
};
use bili_domain::{
    audio::{
        AudioArchive, AudioDetail, AudioOwner, AudioPart, AudioPlayItem, AudioPlayback,
        AudioPlaylist, AudioStat, AudioVolume, AudioWriteReply,
    },
    error::{DomainError, ErrorKind},
};
use bili_grpc::bilibili::{
    app::{archive::middleware::v1::PlayerArgs, listener::v1 as pb},
    pagination::Pagination,
};
use bili_protocol::grpc as paths;
use prost::Message;

use crate::http::{HttpTransport, Idempotency, RequestContext, RetryPolicy};
use crate::protobuf::{Reader, WireType};

#[derive(Debug, Clone)]
pub struct AudioRequestItem {
    pub oid: i64,
    pub sub_id: Vec<i64>,
    pub item_type: i32,
}

#[derive(Debug, Clone)]
pub struct AudioPlaylistRequest {
    pub id: i64,
    pub anchor: Option<AudioRequestItem>,
    pub source: Option<i32>,
    pub extra_id: Option<i64>,
    pub next: Option<String>,
    pub order: i32,
}

impl From<AudioRequestItem> for pb::PlayItem {
    fn from(item: AudioRequestItem) -> Self {
        Self {
            oid: Some(item.oid),
            sub_id: item.sub_id,
            item_type: Some(item.item_type),
        }
    }
}

fn player_args() -> PlayerArgs {
    PlayerArgs {
        qn: Some(80),
        fnval: Some(4048),
        force_host: Some(2),
        voice_balance: Some(1),
        ..Default::default()
    }
}

async fn unary<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    path: &str,
    req: &impl Message,
    retry: RetryPolicy,
) -> Result<Vec<u8>, DomainError> {
    crate::grpc::unary(transport, ctx, path, &req.encode_to_vec(), retry).await
}

fn decode<M: Message + Default>(bytes: &[u8]) -> Result<M, DomainError> {
    M::decode(bytes)
        .map_err(|_| DomainError::new(ErrorKind::Protocol, "音频响应不是合法的 protobuf"))
}

fn item(p: Option<pb::PlayItem>) -> AudioPlayItem {
    let p = p.unwrap_or_default();
    AudioPlayItem {
        item_type: p.item_type.unwrap_or_default(),
        oid: p.oid.unwrap_or_default(),
        sub_id: p.sub_id,
    }
}

fn detail(p: pb::DetailItem) -> AudioDetail {
    let arc = p.arc.unwrap_or_default();
    let owner = p.owner.unwrap_or_default();
    let stat = p.stat.unwrap_or_default();
    let has_associated_oid = p.associated_item.as_ref().is_some_and(|i| i.oid.is_some());
    let has_desc = arc.desc.is_some();
    AudioDetail {
        item: item(p.item),
        arc: AudioArchive {
            title: arc.title.unwrap_or_default(),
            cover: arc.cover.unwrap_or_default(),
            desc: arc.desc.unwrap_or_default(),
            duration: arc.duration.unwrap_or_default(),
            publish: arc.publish.unwrap_or_default(),
            displayed_oid: arc.displayed_oid.unwrap_or_default(),
            copyright: arc.copyright.unwrap_or_default(),
        },
        parts: p
            .parts
            .into_iter()
            .map(|part| AudioPart {
                oid: part.oid.unwrap_or_default(),
                sub_id: part.sub_id.unwrap_or_default(),
                title: part.title.unwrap_or_default(),
            })
            .collect(),
        owner: AudioOwner {
            mid: owner.mid.unwrap_or_default(),
            name: owner.name.clone().unwrap_or_default(),
            avatar: owner.avatar.clone().unwrap_or_default(),
            has_name: owner.name.is_some(),
            has_avatar: owner.avatar.is_some(),
        },
        stat: AudioStat {
            like: stat.like.unwrap_or_default(),
            coin: stat.coin.unwrap_or_default(),
            favourite: stat.favourite.unwrap_or_default(),
            reply: stat.reply.unwrap_or_default(),
            share: stat.share.unwrap_or_default(),
            view: stat.view.unwrap_or_default(),
            has_like: stat.has_like.unwrap_or_default(),
            has_coin: stat.has_coin.unwrap_or_default(),
            has_fav: stat.has_fav.unwrap_or_default(),
        },
        associated_item: item(p.associated_item),
        has_associated_oid,
        has_desc,
    }
}

/// 解码列表字节；独立于网络，供 golden 对照。
pub fn decode_playlist(bytes: &[u8]) -> Result<AudioPlaylist, DomainError> {
    let p: pb::PlaylistResp = decode(bytes)?;
    let pagination = p.pagination_reply.unwrap_or_default();
    Ok(AudioPlaylist {
        reach_start: p.reach_start.unwrap_or_default(),
        reach_end: p.reach_end.unwrap_or_default(),
        prev: pagination.prev.unwrap_or_default(),
        next: pagination.next.unwrap_or_default(),
        list: p.list.into_iter().map(detail).collect(),
    })
}

pub async fn playlist<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    query: AudioPlaylistRequest,
) -> Result<AudioPlaylist, DomainError> {
    let req = pb::PlaylistReq {
        from: query.source,
        id: Some(query.id),
        anchor: Some(query.anchor.map(Into::into).unwrap_or_default()),
        page_opt: None,
        player_args: Some(player_args()),
        extra_id: query.extra_id,
        sort_opt: Some(pb::SortOption {
            order: Some(query.order),
            ..Default::default()
        }),
        pagination: Some(Pagination {
            page_size: Some(20),
            next: query.next,
        }),
    };
    let bytes = unary(
        transport,
        ctx,
        paths::AUDIO_PLAYLIST,
        &req,
        RetryPolicy::Read,
    )
    .await?;
    decode_playlist(&bytes)
}

/// 解析并按已有的音质、CDN 规则选出可播放地址。
pub fn decode_play_url(
    bytes: &[u8],
    preferred_quality: i64,
    cdn_service: &str,
    disable_audio_cdn: bool,
) -> Result<AudioPlayback, DomainError> {
    // Dart 的 map 保留 wire 上首次插入的键顺序；prost 的 BTreeMap 按键排序。
    // 响应若有多个 player_info，仍须取基线的第一个而非最小键。
    let mut entries: Vec<(i64, pb::PlayInfo)> = Vec::new();
    let mut reader = Reader::new(bytes);
    while !reader.is_empty() {
        let tag = reader.read_tag()?;
        if (tag.field, tag.wire) == (4, WireType::LengthDelimited) {
            let mut entry = Reader::new(reader.read_bytes()?);
            let (mut key, mut value) = (0i64, pb::PlayInfo::default());
            while !entry.is_empty() {
                let tag = entry.read_tag()?;
                match (tag.field, tag.wire) {
                    (1, WireType::Varint) => key = entry.read_int64()?,
                    (2, WireType::LengthDelimited) => value = decode(entry.read_bytes()?)?,
                    _ => entry.skip(tag.wire)?,
                }
            }
            if let Some((_, old)) = entries.iter_mut().find(|(k, _)| *k == key) {
                *old = value;
            } else {
                entries.push((key, value));
            }
        } else {
            reader.skip(tag.wire)?;
        }
    }
    let Some((_, info)) = entries.into_iter().next() else {
        return Ok(AudioPlayback {
            url: None,
            volume: None,
        });
    };
    let volume = info.volume.map(|v| AudioVolume {
        measured_i: v.measured_i.unwrap_or_default(),
        measured_lra: v.measured_lra.unwrap_or_default(),
        measured_tp: v.measured_tp.unwrap_or_default(),
        measured_threshold: v.measured_threshold.unwrap_or_default(),
        target_offset: v.target_offset.unwrap_or_default(),
        target_i: v.target_i.unwrap_or_default(),
        target_tp: v.target_tp.unwrap_or_default(),
    });
    let urls = match info.info {
        Some(pb::play_info::Info::PlayDash(dash)) => {
            let ids: Vec<_> = dash
                .audio
                .iter()
                .map(|a| i64::from(a.id.unwrap_or_default()))
                .collect();
            let Some(index) = find_target_media_index(&ids, preferred_quality) else {
                return Ok(AudioPlayback { url: None, volume });
            };
            let a = &dash.audio[index];
            std::iter::once(a.base_url.clone().unwrap_or_default())
                .chain(a.backup_url.iter().cloned())
                .collect::<Vec<_>>()
        }
        Some(pb::play_info::Info::PlayUrl(url)) => {
            let Some(first) = url.durl.first() else {
                return Ok(AudioPlayback { url: None, volume });
            };
            std::iter::once(first.url.clone().unwrap_or_default())
                .chain(first.backup_url.iter().cloned())
                .collect::<Vec<_>>()
        }
        None => return Ok(AudioPlayback { url: None, volume }),
    };
    let selected = pick_cdn_url(
        &urls,
        CdnService::from_key(cdn_service),
        false,
        disable_audio_cdn,
    );
    Ok(AudioPlayback {
        url: Some(selected),
        volume,
    })
}

pub async fn play_url<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    item: AudioRequestItem,
    preferred_quality: i64,
    cdn_service: &str,
    disable_audio_cdn: bool,
) -> Result<AudioPlayback, DomainError> {
    let req = pb::PlayUrlReq {
        item: Some(item.into()),
        player_args: Some(player_args()),
    };
    let bytes = unary(
        transport,
        ctx,
        paths::AUDIO_PLAY_URL,
        &req,
        RetryPolicy::Read,
    )
    .await?;
    decode_play_url(&bytes, preferred_quality, cdn_service, disable_audio_cdn)
}

pub async fn thumb_up<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    item: AudioRequestItem,
    like: bool,
) -> Result<AudioWriteReply, DomainError> {
    let req = pb::ThumbUpReq {
        item: Some(item.into()),
        action: Some(if like {
            pb::thumb_up_req::ThumbType::Like as i32
        } else {
            pb::thumb_up_req::ThumbType::CancelLike as i32
        }),
    };
    let bytes = unary(
        transport,
        ctx,
        paths::AUDIO_THUMB_UP,
        &req,
        Idempotency::Idempotent.retry_policy(),
    )
    .await?;
    let p: pb::ThumbUpResp = decode(&bytes)?;
    Ok(AudioWriteReply {
        message: p.message.unwrap_or_default(),
        coin_ok: false,
    })
}

pub async fn triple_like<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    item: AudioRequestItem,
) -> Result<AudioWriteReply, DomainError> {
    let req = pb::TripleLikeReq {
        item: Some(item.into()),
    };
    let bytes = unary(
        transport,
        ctx,
        paths::AUDIO_TRIPLE_LIKE,
        &req,
        Idempotency::Verifiable.retry_policy(),
    )
    .await?;
    let p: pb::TripleLikeResp = decode(&bytes)?;
    Ok(AudioWriteReply {
        message: p.message.unwrap_or_default(),
        coin_ok: p.coin_ok.unwrap_or_default(),
    })
}

pub async fn coin_add<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    item: AudioRequestItem,
    num: i32,
    thumb_up: bool,
) -> Result<AudioWriteReply, DomainError> {
    let req = pb::CoinAddReq {
        item: Some(item.into()),
        num: Some(num),
        thumb_up: Some(thumb_up),
    };
    let bytes = unary(
        transport,
        ctx,
        paths::AUDIO_COIN_ADD,
        &req,
        Idempotency::Verifiable.retry_policy(),
    )
    .await?;
    let p: pb::CoinAddResp = decode(&bytes)?;
    Ok(AudioWriteReply {
        message: p.message.unwrap_or_default(),
        coin_ok: false,
    })
}
