//! 直播长连接的生命周期与有界批次。UI 不参与协议和候选地址选择。
//!
//! 通过现有 reqwest 的 HTTP Upgrade 复用代理、证书和 TLS 根配置；
//! 升级完成后不设整个连接的 HTTP 超时，只对握手、写入和鉴权限时。
//! 断线即终止，与 Dart 基线一致，不悄悄增加自动重连。
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::live_danmaku::{LiveBlockRules, LiveEvent};
use futures_util::{SinkExt, StreamExt};
use tokio::task::JoinHandle;
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{
        Message,
        handshake::{client::generate_key, derive_accept_key},
        protocol::{Role, WebSocketConfig},
    },
};

use crate::http::TransportConfig;
use crate::live_danmaku::{auth_code, auth_packet, decode_packets, heartbeat_packet, parse_event};

const TIMEOUT: Duration = Duration::from_secs(10);
const CAPACITY: usize = 2048;
const BATCH_SIZE: usize = 512;
const QUEUE_BYTES: usize = 8 * 1024 * 1024;
const MAX_MESSAGE: usize = 8 * 1024 * 1024;

type Socket = WebSocketStream<reqwest::Upgraded>;

fn failure(kind: ErrorKind, message: &str) -> DomainError {
    // 不透传网络库的错误：其中可能带含凭据的 URL 或远端响应。
    DomainError::new(kind, message)
}

/// 所有尝试共用 HTTP 配置；WebSocket 握手固定 HTTP/1.1。
pub async fn connect(server: &str, config: &TransportConfig) -> Result<Socket, DomainError> {
    let mut url =
        url::Url::parse(server).map_err(|_| failure(ErrorKind::Protocol, "直播弹幕地址无效"))?;
    let scheme = match url.scheme() {
        "ws" => "http",
        "wss" => "https",
        _ => return Err(failure(ErrorKind::Protocol, "直播弹幕地址协议无效")),
    };
    url.set_scheme(scheme)
        .map_err(|_| failure(ErrorKind::Protocol, "直播弹幕地址无效"))?;
    let key = generate_key();
    // version() 只指定请求版本；TLS ALPN 也必须限制为 HTTP/1.1，
    // 否则开启 HTTP/2 的用户可能协商 h2，无法使用 101 Upgrade。
    let config = TransportConfig {
        http1_only: true,
        ..config.clone()
    };
    let request = crate::transport_reqwest::client(&config)?
        .get(url)
        .version(reqwest::Version::HTTP_11)
        .header("connection", "Upgrade")
        .header("upgrade", "websocket")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", &key);
    let upgrade = async {
        let response = request
            .send()
            .await
            .map_err(|_| failure(ErrorKind::Network, "直播弹幕握手失败"))?;
        let headers = response.headers();
        let is_upgrade = |name: &str, expected: &str| {
            headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| {
                    v.split(',')
                        .any(|v| v.trim().eq_ignore_ascii_case(expected))
                })
        };
        if response.status() != reqwest::StatusCode::SWITCHING_PROTOCOLS
            || !is_upgrade("connection", "upgrade")
            || !is_upgrade("upgrade", "websocket")
            || headers
                .get("sec-websocket-accept")
                .and_then(|v| v.to_str().ok())
                != Some(derive_accept_key(key.as_bytes()).as_str())
        {
            return Err(failure(
                ErrorKind::Protocol,
                "直播弹幕 WebSocket 握手响应无效",
            ));
        }
        response
            .upgrade()
            .await
            .map_err(|_| failure(ErrorKind::Network, "直播弹幕连接升级失败"))
    };
    let stream = tokio::time::timeout(TIMEOUT, upgrade)
        .await
        .map_err(|_| failure(ErrorKind::Network, "直播弹幕握手超时"))??;
    let ws_config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE))
        .max_frame_size(Some(MAX_MESSAGE));
    Ok(WebSocketStream::from_raw_socket(stream, Role::Client, Some(ws_config)).await)
}

#[derive(Debug, Default)]
struct State {
    events: VecDeque<(LiveEvent, usize)>,
    bytes: usize,
    dropped: u64,
    error: Option<DomainError>,
    ended: bool,
    closed: bool,
    rules: LiveBlockRules,
}

impl State {
    fn push(&mut self, event: LiveEvent, bytes: usize) {
        if self.closed {
            return;
        }
        if event
            .danmaku
            .as_ref()
            .is_some_and(|m| self.rules.is_blocked(&m.text, m.mid))
        {
            return;
        }
        // 保留最新的房间消息；终止错误另存，不会被队列溢出挤掉。
        while self.events.len() >= CAPACITY || self.bytes.saturating_add(bytes) > QUEUE_BYTES {
            let Some((_, removed)) = self.events.pop_front() else {
                self.dropped = self.dropped.saturating_add(1);
                return;
            };
            self.bytes -= removed;
            self.dropped = self.dropped.saturating_add(1);
        }
        self.bytes += bytes;
        self.events.push_back((event, bytes));
    }
}

#[derive(Debug)]
pub struct SocketBatch {
    pub events: Vec<LiveEvent>,
    pub dropped: u64,
    pub ended: bool,
    pub error: Option<DomainError>,
}

/// Drop 与 close 都取消连接中的 Future，不等待远端 close 握手。
#[derive(Debug)]
pub struct SocketSession {
    state: Arc<Mutex<State>>,
    task: JoinHandle<()>,
}

impl SocketSession {
    /// 同步取得句柄，因此连接尚未完成时也可以取消。
    pub fn start(
        servers: Vec<String>,
        room_id: i64,
        uid: i64,
        token: String,
        config: TransportConfig,
    ) -> Self {
        let state = Arc::new(Mutex::new(State::default()));
        let target = state.clone();
        let task = tokio::spawn(async move {
            let result = run(servers, room_id, uid, token, config, target.clone()).await;
            let mut state = target.lock().unwrap_or_else(|e| e.into_inner());
            if !state.closed {
                state.ended = true;
                state.error = result.err();
            }
        });
        Self { state, task }
    }

    pub fn poll(&self) -> SocketBatch {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        SocketBatch {
            events: if state.closed {
                Vec::new()
            } else {
                let count = state.events.len().min(BATCH_SIZE);
                let drained: Vec<_> = state.events.drain(..count).collect();
                state.bytes -= drained.iter().map(|(_, bytes)| bytes).sum::<usize>();
                drained.into_iter().map(|(event, _)| event).collect()
            },
            dropped: std::mem::take(&mut state.dropped),
            ended: (state.ended || state.closed) && state.events.is_empty(),
            error: if state.events.is_empty() {
                state.error.clone()
            } else {
                None
            },
        }
    }

    pub fn set_rules(&self, rules: LiveBlockRules) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.events.retain(|(event, _)| {
            event
                .danmaku
                .as_ref()
                .is_none_or(|m| !rules.is_blocked(&m.text, m.mid))
        });
        state.bytes = state.events.iter().map(|(_, bytes)| bytes).sum();
        state.rules = rules;
    }

    pub fn close(&mut self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.closed = true;
        state.events.clear();
        state.bytes = 0;
        state.error = None;
        self.task.abort();
    }

    /// 等待取消生效，仅用于需要显式等待任务释放的调用方。
    pub async fn is_finished(&mut self) -> bool {
        (&mut self.task).await.is_err() || self.task.is_finished()
    }
}

impl Drop for SocketSession {
    fn drop(&mut self) {
        self.close();
    }
}

async fn send(socket: &mut Socket, bytes: Vec<u8>) -> Result<(), DomainError> {
    tokio::time::timeout(TIMEOUT, socket.send(Message::Binary(bytes.into())))
        .await
        .map_err(|_| failure(ErrorKind::Network, "直播弹幕写入超时"))?
        .map_err(|_| failure(ErrorKind::Network, "直播弹幕写入失败"))
}

async fn run(
    servers: Vec<String>,
    room_id: i64,
    uid: i64,
    token: String,
    config: TransportConfig,
    state: Arc<Mutex<State>>,
) -> Result<(), DomainError> {
    let mut socket = None;
    for server in servers {
        if let Ok(candidate) = connect(&server, &config).await {
            socket = Some(candidate);
            break;
        }
    }
    let mut socket =
        socket.ok_or_else(|| failure(ErrorKind::Network, "所有直播弹幕地址连接失败"))?;
    send(&mut socket, auth_packet(room_id, uid, &token)?).await?;
    // 认证包发送后不再持有 token；保留的会话只有消息、规则与任务句柄。
    drop(token);
    let auth_deadline = tokio::time::sleep(TIMEOUT);
    tokio::pin!(auth_deadline);
    let mut authenticated = false;
    let mut sequence = 1;
    let mut heartbeat = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_secs(30),
        Duration::from_secs(30),
    );
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = &mut auth_deadline, if !authenticated => return Err(failure(ErrorKind::Auth, "直播弹幕鉴权超时")),
            _ = heartbeat.tick(), if authenticated => {
                send(&mut socket, heartbeat_packet(sequence)).await?;
                sequence = sequence.wrapping_add(1);
            },
            message = socket.next() => {
                let Some(message) = message else { return Err(failure(ErrorKind::Network, "直播弹幕连接已断开")); };
                match message.map_err(|_| failure(ErrorKind::Network, "直播弹幕接收失败"))? {
                    Message::Binary(bytes) => {
                        for packet in decode_packets(&bytes)? {
                            match packet.operation {
                                8 => {
                                    let code = auth_code(&packet.body)?;
                                    if code != 0 { return Err(DomainError::with_code(ErrorKind::Auth, "直播弹幕鉴权被拒绝", code)); }
                                    if !authenticated {
                                        authenticated = true;
                                        heartbeat.reset_after(Duration::from_secs(30));
                                    }
                                },
                                5 if authenticated => {
                                    // 单条无效消息不影响同一个压缩包中的后续消息。
                                    if let Ok(Some(event)) = parse_event(&packet.body, room_id) {
                                        state.lock().unwrap_or_else(|e| e.into_inner()).push(event, packet.body.len());
                                    }
                                },
                                _ => {},
                            }
                        }
                    },
                    Message::Ping(_) => {
                        tokio::time::timeout(TIMEOUT, socket.flush()).await.map_err(|_| failure(ErrorKind::Network, "直播弹幕 pong 超时"))?.map_err(|_| failure(ErrorKind::Network, "直播弹幕 pong 失败"))?;
                    },
                    Message::Close(_) => return Ok(()),
                    _ => {},
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bili_domain::live_danmaku::LiveEventKind;

    fn event(n: usize) -> LiveEvent {
        LiveEvent {
            kind: LiveEventKind::RoomChange,
            danmaku: None,
            super_chat: None,
            text: Some(n.to_string()),
            count: None,
        }
    }

    #[test]
    fn queue_payload_budget_evicts_before_item_capacity() {
        let mut state = State::default();
        state.push(event(1), QUEUE_BYTES / 2 + 1);
        state.push(event(2), QUEUE_BYTES / 2 + 1);
        assert_eq!(state.events.len(), 1);
        assert_eq!(state.dropped, 1);
        assert!(state.bytes <= QUEUE_BYTES);
        state.push(event(3), QUEUE_BYTES + 1);
        assert!(state.events.is_empty());
        assert_eq!(state.dropped, 3);
        assert_eq!(state.bytes, 0);
    }

    #[test]
    fn queue_is_bounded_and_terminal_failure_survives_overflow() {
        let mut state = State {
            error: Some(failure(ErrorKind::Network, "closed")),
            ..State::default()
        };
        for n in 0..CAPACITY + 9 {
            state.push(event(n), 10);
        }
        assert_eq!(state.events.len(), CAPACITY);
        assert_eq!(state.dropped, 9);
        assert_eq!(state.events.front().unwrap().0.text.as_deref(), Some("9"));
        assert!(state.error.is_some());
        state.closed = true;
        state.push(event(99), 10);
        assert_eq!(state.events.len(), CAPACITY);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod batch_tests {
    use super::*;
    use bili_domain::live_danmaku::LiveEventKind;

    #[tokio::test]
    async fn queued_messages_drain_before_terminal_error_is_reported() {
        let mut session =
            SocketSession::start(vec![], 1, 0, String::new(), TransportConfig::default());
        session.task.abort();
        {
            let mut state = session.state.lock().unwrap();
            state.ended = true;
            state.error = Some(failure(ErrorKind::Network, "closed"));
            for _ in 0..BATCH_SIZE + 1 {
                state.push(
                    LiveEvent {
                        kind: LiveEventKind::RoomChange,
                        danmaku: None,
                        super_chat: None,
                        text: Some("title".into()),
                        count: None,
                    },
                    100,
                );
            }
        }
        let first = session.poll();
        assert_eq!(first.events.len(), BATCH_SIZE);
        assert!(!first.ended);
        assert!(first.error.is_none());
        let last = session.poll();
        assert_eq!(last.events.len(), 1);
        assert!(last.ended);
        assert!(last.error.is_some());
        session.close();
        assert!(session.poll().events.is_empty());
        assert!(session.poll().error.is_none());
    }
}
