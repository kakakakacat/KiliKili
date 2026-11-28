//! 从 GrpcHeaders 逐字迁移的协议常量，刻意保留基线的头名与设备形态。
pub const APP_ID: i32 = 5;
pub const LOCALE_LANGUAGE: &str = "zh";
pub const LOCALE_REGION: &str = "CN";
pub const LOCALE_SCRIPT: &str = "Hans";
pub const TIMEZONE: &str = "Asia/Shanghai";
pub const NETWORK_TYPE: i32 = 1;
pub const OS_VERSION: &str = "15";
pub const FAWKES_ENV: &str = "prod";
pub const BUILD: i32 = 2_001_100;
pub const VERSION_NAME: &str = "2.0.1";
pub const CHANNEL: &str = "master";
pub const MOBI_APP: &str = "android_hd";
pub const DEVICE: &str = "android";
pub const TRACE_ID: &str = "11111111111111111111111111111111:1111111111111111:0:0";
pub const USER_AGENT: &str = "Mozilla/5.0 BiliDroid/2.0.1 (bbcallen@gmail.com) os/android model/android_hd mobi_app/android_hd build/2001100 channel/master innerVer/2001100 osVer/15 network/2";
pub const DM_SEG_MOBILE: &str = "/bilibili.community.service.dm.v1.DM/DmSegMobile";
pub const DM_VIEW: &str = "/bilibili.community.service.dm.v1.DM/DmView";

/// 评论的 gRPC 方法路径。照抄基线 `lib/grpc/url.dart` 的 `GrpcUrl.reply*`。
///
/// 与 `grpc_space` 里那条 `interface` / `interfaces` 对不上的服务名不同，
/// 评论这组的服务名与 proto 包名一致。
pub const REPLY_MAIN_LIST: &str = "/bilibili.main.community.reply.v1.Reply/MainList";
pub const REPLY_DETAIL_LIST: &str = "/bilibili.main.community.reply.v1.Reply/DetailList";
pub const REPLY_FOLD_LIST: &str = "/bilibili.main.community.reply.v1.Reply/FoldList";
pub const REPLY_DIALOG_LIST: &str = "/bilibili.main.community.reply.v1.Reply/DialogList";
pub const REPLY_SEARCH_ITEM: &str = "/bilibili.main.community.reply.v1.Reply/SearchItem";
pub const REPLY_TRANSLATE: &str = "/bilibili.main.community.reply.v1.Reply/TranslateReply";

/// 私信（IM）的 gRPC 方法路径。照抄基线 `lib/grpc/url.dart` 的 `GrpcUrl`
/// im 那一段（S12 续单 / issue #63）。
///
/// **服务名与 proto 包名对不上，两个都要注意：**
///
/// - `ImInterface` 这组的服务名是 `bilibili.im.interface.v1`（interface，
///   **单数**），而消息的 package 是 `bilibili.im.interfaces.v1`（复数，
///   来自描述符文件路径 `bilibili/im/interfaces/v1.proto`）。
/// - `im` 这组的服务名末段是小写的 `im`，不是驼峰的 `Im`。
///
/// service 定义不在描述符里（ADR-012 决策四），所以这两处都没有任何东西
/// 替我们校验——拼错的表现是 gRPC 状态 12（UNIMPLEMENTED），而那个码看着
/// 像「服务端下线了这个接口」。与 `grpc_space` 那条同类。
pub const IM_SEND_MSG: &str = "/bilibili.im.interface.v1.ImInterface/SendMsg";
pub const IM_SHARE_LIST: &str = "/bilibili.im.interface.v1.ImInterface/ShareList";
pub const IM_SYNC_FETCH_SESSION_MSGS: &str =
    "/bilibili.im.interface.v1.ImInterface/SyncFetchSessionMsgs";
pub const IM_GET_TOTAL_UNREAD: &str = "/bilibili.im.interface.v1.ImInterface/GetTotalUnread";
pub const IM_SESSION_DETAIL: &str = "/bilibili.im.interface.v1.ImInterface/SessionDetail";

pub const IM_SESSION_MAIN: &str = "/bilibili.app.im.v1.im/SessionMain";
pub const IM_SESSION_SECONDARY: &str = "/bilibili.app.im.v1.im/SessionSecondary";
pub const IM_CLEAR_UNREAD: &str = "/bilibili.app.im.v1.im/ClearUnread";
pub const IM_SESSION_UPDATE: &str = "/bilibili.app.im.v1.im/SessionUpdate";
pub const IM_PIN_SESSION: &str = "/bilibili.app.im.v1.im/PinSession";
pub const IM_UNPIN_SESSION: &str = "/bilibili.app.im.v1.im/UnpinSession";
pub const IM_DELETE_SESSION_LIST: &str = "/bilibili.app.im.v1.im/DeleteSessionList";
pub const IM_GET_SETTINGS: &str = "/bilibili.app.im.v1.im/GetImSettings";
pub const IM_SET_SETTINGS: &str = "/bilibili.app.im.v1.im/SetImSettings";
pub const IM_KEYWORD_BLOCKING_LIST: &str = "/bilibili.app.im.v1.im/KeywordBlockingList";
pub const IM_KEYWORD_BLOCKING_ADD: &str = "/bilibili.app.im.v1.im/KeywordBlockingAdd";
pub const IM_KEYWORD_BLOCKING_DELETE: &str = "/bilibili.app.im.v1.im/KeywordBlockingDelete";

pub const AUDIO_PLAY_URL: &str = "/bilibili.app.listener.v1.Listener/PlayURL";
pub const AUDIO_PLAYLIST: &str = "/bilibili.app.listener.v1.Listener/Playlist";
pub const AUDIO_THUMB_UP: &str = "/bilibili.app.listener.v1.Listener/ThumbUp";
pub const AUDIO_TRIPLE_LIKE: &str = "/bilibili.app.listener.v1.Listener/TripleLike";
pub const AUDIO_COIN_ADD: &str = "/bilibili.app.listener.v1.Listener/CoinAdd";
