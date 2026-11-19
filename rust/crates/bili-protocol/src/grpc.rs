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
