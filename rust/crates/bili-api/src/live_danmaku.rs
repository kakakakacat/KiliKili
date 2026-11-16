//! 直播弹幕协议、响应映射与只读请求。解压预算跨嵌套共享，避免压缩炸弹。
use crate::{
    envelope::unwrap_envelope,
    http::{HttpTransport, RequestContext},
    request::PreparedRequest,
};
use bili_domain::{
    error::{DomainError, ErrorKind},
    live::LiveMedal,
    live_danmaku::*,
};
use serde_json::Value;
use std::io::Read;
const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_PACKETS: usize = 4096;
const MAX_DEPTH: usize = 4;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LivePacket {
    pub operation: u32,
    pub body: Vec<u8>,
}
fn invalid() -> DomainError {
    DomainError::new(ErrorKind::Protocol, "直播弹幕数据格式无效或超过限制")
}
fn json(bytes: &[u8]) -> Result<Value, DomainError> {
    serde_json::from_slice(bytes).map_err(|_| invalid())
}
pub fn encode_packet(operation: u32, body: &[u8]) -> Result<Vec<u8>, DomainError> {
    if body.len() > MAX_BYTES - 16 {
        return Err(invalid());
    }
    let mut out = Vec::with_capacity(body.len() + 16);
    out.extend_from_slice(&((body.len() + 16) as u32).to_be_bytes());
    out.extend_from_slice(&16u16.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&operation.to_be_bytes());
    out.extend_from_slice(&1u32.to_be_bytes());
    out.extend_from_slice(body);
    Ok(out)
}
pub fn auth_packet(room_id: i64, uid: i64, token: &str) -> Result<Vec<u8>, DomainError> {
    encode_packet(7,&serde_json::to_vec(&serde_json::json!({"roomid":room_id,"uid":uid,"protover":3,"platform":"web","type":2,"key":token})).map_err(|_|invalid())?)
}
pub fn heartbeat_packet(seq: u32) -> Vec<u8> {
    let mut out = vec![0, 0, 0, 16, 0, 16, 0, 1, 0, 0, 0, 2];
    out.extend_from_slice(&seq.to_be_bytes());
    out
}
pub fn auth_code(body: &[u8]) -> Result<i64, DomainError> {
    json(body)?["code"].as_i64().ok_or_else(invalid)
}
pub fn popularity(body: &[u8]) -> Result<u32, DomainError> {
    Ok(u32::from_be_bytes(body.try_into().map_err(|_| invalid())?))
}
pub fn decode_packets(bytes: &[u8]) -> Result<Vec<LivePacket>, DomainError> {
    let mut packets = Vec::new();
    let mut budget = MAX_BYTES;
    decode(bytes, 0, &mut budget, &mut packets)?;
    Ok(packets)
}
fn decode(
    mut bytes: &[u8],
    depth: usize,
    budget: &mut usize,
    out: &mut Vec<LivePacket>,
) -> Result<(), DomainError> {
    if bytes.len() > MAX_BYTES || depth > MAX_DEPTH {
        return Err(invalid());
    }
    while !bytes.is_empty() {
        if bytes.len() < 16 {
            return Err(invalid());
        }
        let total = u32::from_be_bytes(bytes[0..4].try_into().map_err(|_| invalid())?) as usize;
        let header = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
        let version = u16::from_be_bytes([bytes[6], bytes[7]]);
        let operation = u32::from_be_bytes(bytes[8..12].try_into().map_err(|_| invalid())?);
        if header < 16 || total < header || total > bytes.len() || total > MAX_BYTES {
            return Err(invalid());
        }
        let body = &bytes[header..total];
        match version {
            0 | 1 => {
                if out.len() >= MAX_PACKETS {
                    return Err(invalid());
                }
                out.push(LivePacket {
                    operation,
                    body: body.to_vec(),
                });
            }
            2 | 3 => {
                let reader: Box<dyn Read> = if version == 2 {
                    Box::new(flate2::read::ZlibDecoder::new(body))
                } else {
                    Box::new(brotli::Decompressor::new(body, 4096))
                };
                let mut expanded = Vec::new();
                reader
                    .take((*budget + 1) as u64)
                    .read_to_end(&mut expanded)
                    .map_err(|_| invalid())?;
                if expanded.len() > *budget {
                    return Err(invalid());
                }
                *budget -= expanded.len();
                decode(&expanded, depth + 1, budget, out)?;
            }
            _ => return Err(invalid()),
        }
        bytes = &bytes[total..];
    }
    Ok(())
}
pub fn build_token_request(room_id: i64, mixin_key: &str, wts: i64) -> PreparedRequest {
    let mut query = vec![
        ("id".into(), room_id.to_string()),
        ("web_location".into(), "444.8".into()),
        ("wts".into(), wts.to_string()),
    ];
    let sign = bili_protocol::wbi::sign(&query, mixin_key);
    query.push(("w_rid".into(), sign));
    PreparedRequest {
        path: "/xlive/web-room/v1/index/getDanmuInfo",
        query,
    }
}
pub fn build_prefetch_request(room_id: i64) -> PreparedRequest {
    PreparedRequest {
        path: "/xlive/web-room/v1/dM/gethistory",
        query: vec![("roomid".into(), room_id.to_string())],
    }
}
pub fn build_super_chat_request(room_id: i64) -> PreparedRequest {
    PreparedRequest {
        path: "/av/v1/SuperChat/getMessageList",
        query: vec![("room_id".into(), room_id.to_string())],
    }
}
pub fn parse_token(body: &str) -> Result<LiveDanmakuToken, DomainError> {
    let v: Value = unwrap_envelope(body)?;
    let token = req_str(&v, "token")?;
    let hosts = v["host_list"].as_array().ok_or_else(invalid)?;
    let mut servers = Vec::new();
    for h in hosts {
        if let (Some(host), Some(port)) = (h["host"].as_str(), h["wss_port"].as_u64())
            && !host.is_empty()
            && port > 0
            && port <= 65535
            && host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        {
            servers.push(format!("wss://{host}:{port}/sub"));
        }
    }
    if servers.is_empty() {
        return Err(invalid());
    }
    Ok(LiveDanmakuToken { token, servers })
}
pub async fn fetch_token<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    room_id: i64,
    mixin_key: &str,
    wts: i64,
) -> Result<LiveDanmakuToken, DomainError> {
    parse_token(
        &crate::client::send(
            transport,
            ctx,
            &build_token_request(room_id, mixin_key, wts),
        )
        .await?,
    )
}
pub async fn fetch_prefetch<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    room_id: i64,
    user_agent: &str,
) -> Result<Vec<LiveDanmakuMessage>, DomainError> {
    let mut ctx = ctx.clone();
    // 基线 Options.headers 覆盖默认值，不能把同名头追加成两个值。
    ctx.headers.retain(|(key, _)| {
        !key.eq_ignore_ascii_case("referer") && !key.eq_ignore_ascii_case("user-agent")
    });
    ctx.headers.push((
        "referer".into(),
        format!("https://live.bilibili.com/{room_id}"),
    ));
    ctx.headers.push(("user-agent".into(), user_agent.into()));
    parse_prefetch(&crate::client::send(transport, &ctx, &build_prefetch_request(room_id)).await?)
}
pub async fn fetch_super_chat<T: HttpTransport>(
    transport: &T,
    ctx: &RequestContext,
    room_id: i64,
) -> Result<Vec<LiveSuperChat>, DomainError> {
    parse_super_chat(
        &crate::client::send(transport, ctx, &build_super_chat_request(room_id)).await?,
        room_id,
    )
}
fn int(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_str()?.parse().ok())
}
fn string(v: &Value) -> Option<String> {
    v.as_str()
        .map(str::to_owned)
        .or_else(|| v.as_i64().map(|n| n.to_string()))
}
fn req_str(v: &Value, key: &str) -> Result<String, DomainError> {
    string(&v[key]).ok_or_else(invalid)
}
fn req_int(v: &Value, key: &str) -> Result<i64, DomainError> {
    int(&v[key]).ok_or_else(invalid)
}
fn medal(v: &Value) -> Option<LiveMedal> {
    if int(&v["is_light"]) != Some(1) {
        return None;
    }
    Some(LiveMedal {
        name: string(&v["name"]),
        level: int(&v["level"]),
        id: int(&v["id"]),
        ruid: int(&v["ruid"]),
        color_start: string(&v["v2_medal_color_start"]),
        color_text: string(&v["v2_medal_color_text"]),
    })
}
fn emote(v: &Value) -> Result<LiveDanmakuEmote, DomainError> {
    let width = v["width"].as_f64().ok_or_else(invalid)?;
    let unique = req_str(v, "emoticon_unique")?;
    Ok(LiveDanmakuEmote {
        url: req_str(v, "url")?,
        is_official: unique.starts_with("official_"),
        emoticon_unique: unique,
        width,
        height: v["height"].as_f64().unwrap_or(width),
    })
}
fn message(
    v: &Value,
    user: &Value,
    extra: &Value,
    check: &Value,
    uemote: &Value,
    reply: &Value,
) -> Result<LiveDanmakuMessage, DomainError> {
    let mut emotes = Vec::new();
    if let Some(map) = extra["emots"].as_object() {
        for (key, e) in map {
            emotes.push(LiveDanmakuEmoteEntry {
                key: key.clone(),
                emote: emote(e)?,
            });
        }
    }
    let text = req_str(v, "text")?;
    let segments = segments(&text, &emotes);
    Ok(LiveDanmakuMessage {
        id: req_str(extra, "id_str")?,
        mid: req_int(user, "uid")?,
        dm_type: req_int(extra, "dm_type")?,
        ts: req_int(check, "ts")?,
        ct: req_str(check, "ct")?,
        color: int(&extra["color"]).unwrap_or(16777215),
        mode: int(&extra["mode"]).unwrap_or(1),
        name: req_str(&user["base"], "name")?,
        text,
        emotes,
        segments,
        uemote: if uemote.is_object()
            && uemote["emoticon_unique"]
                .as_str()
                .is_some_and(|s| !s.is_empty())
        {
            Some(emote(uemote)?)
        } else {
            None
        },
        reply: int(&reply["reply_mid"])
            .filter(|n| *n != 0)
            .map(|mid| LiveDanmakuReply {
                mid,
                name: string(&reply["reply_uname"]),
            }),
        medal: medal(&user["medal"]),
    })
}
fn segments(text: &str, emotes: &[LiveDanmakuEmoteEntry]) -> Vec<LiveMessageSegment> {
    let mut result = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let next = emotes
            .iter()
            .filter(|e| !e.key.is_empty())
            .filter_map(|e| rest.find(&e.key).map(|p| (p, e)))
            .min_by_key(|(p, _)| *p);
        if let Some((pos, e)) = next {
            if pos > 0 {
                result.push(LiveMessageSegment {
                    text: rest[..pos].into(),
                    emote: None,
                })
            }
            result.push(LiveMessageSegment {
                text: e.key.clone(),
                emote: Some(e.emote.clone()),
            });
            rest = &rest[pos + e.key.len()..];
        } else {
            result.push(LiveMessageSegment {
                text: rest.into(),
                emote: None,
            });
            break;
        }
    }
    result
}
pub fn parse_prefetch(body: &str) -> Result<Vec<LiveDanmakuMessage>, DomainError> {
    let v: Value = unwrap_envelope(body)?;
    let mut out = Vec::new();
    if let Some(room) = v["room"].as_array() {
        for item in room {
            out.push(message(
                item,
                &item["user"],
                item,
                &item["check_info"],
                &item["emoticon"],
                &item["reply"],
            )?);
        }
    }
    Ok(out)
}
pub fn parse_super_chat(body: &str, room_id: i64) -> Result<Vec<LiveSuperChat>, DomainError> {
    let v: Value = unwrap_envelope(body)?;
    v["list"]
        .as_array()
        .map(|items| items.iter().map(|v| super_chat(v, room_id)).collect())
        .unwrap_or_else(|| Ok(Vec::new()))
}
fn super_chat(v: &Value, room_id: i64) -> Result<LiveSuperChat, DomainError> {
    let u = &v["user_info"];
    Ok(LiveSuperChat {
        id: req_int(v, "id")?,
        uid: req_int(v, "uid")?,
        price: req_int(v, "price")?,
        background_image: string(&v["background_image"]).filter(|s| !s.is_empty()),
        background_color: string(&v["background_color"]).unwrap_or_else(|| "#EDF5FF".into()),
        background_bottom_color: string(&v["background_bottom_color"])
            .unwrap_or_else(|| "#2A60B2".into()),
        background_price_color: string(&v["background_price_color"])
            .unwrap_or_else(|| "#7497CD".into()),
        message_font_color: string(&v["message_font_color"]).unwrap_or_else(|| "#FFFFFF".into()),
        start_time: req_int(v, "start_time")?,
        end_time: req_int(v, "end_time")?,
        message: req_str(v, "message")?,
        token: req_str(v, "token")?,
        ts: req_int(v, "ts")?,
        user_info: LiveSuperChatUser {
            face: req_str(u, "face")?,
            face_frame: string(&u["face_frame"]),
            uname: req_str(u, "uname")?,
            name_color: string(&u["name_color"]).unwrap_or_else(|| "#666666".into()),
        },
        medal: medal(&v["uinfo"]["medal"]),
        room_id,
    })
}
pub fn parse_event(body: &[u8], room_id: i64) -> Result<Option<LiveEvent>, DomainError> {
    let v = json(body)?;
    let cmd = v["cmd"]
        .as_str()
        .ok_or_else(invalid)?
        .split(':')
        .next()
        .ok_or_else(invalid)?;
    let mut event = LiveEvent {
        kind: LiveEventKind::Danmaku,
        danmaku: None,
        super_chat: None,
        text: None,
        count: None,
    };
    match cmd {
        "DANMU_MSG" => {
            let info = &v["info"];
            let first = &info[0];
            let content = &first[15];
            let extra = json(content["extra"].as_str().ok_or_else(invalid)?.as_bytes())?;
            event.danmaku = Some(message(
                &serde_json::json!({"text":info[1]}),
                &content["user"],
                &extra,
                &info[9],
                &first[13],
                &extra,
            )?);
        }
        "SUPER_CHAT_MESSAGE" => {
            event.kind = LiveEventKind::SuperChat;
            event.super_chat = Some(super_chat(&v["data"], room_id)?);
        }
        "WATCHED_CHANGE" => {
            event.kind = LiveEventKind::WatchedChange;
            event.text = Some(req_str(&v["data"], "text_large")?);
        }
        "ONLINE_RANK_COUNT" => {
            event.kind = LiveEventKind::OnlineRankCount;
            event.count = Some(req_int(&v["data"], "count")?);
        }
        "ROOM_CHANGE" => {
            event.kind = LiveEventKind::RoomChange;
            event.text = Some(req_str(&v["data"], "title")?);
        }
        _ => return Ok(None),
    }
    Ok(Some(event))
}
