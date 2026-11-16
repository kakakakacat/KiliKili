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
