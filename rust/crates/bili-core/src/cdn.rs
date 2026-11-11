//! 取流地址的 CDN 选路。
//!
//! 基线实现在 `lib/utils/video_utils.dart` 的 `getCdnUrl`：一组候选地址
//! （主地址 + 备用地址）进去，一个真正要播的地址出来。中间那几条分支
//! （镜像站改 host、mcdn 走 tf 代理、`szbdyd.com` 换回源站）都是**业务
//! 规则**——换掉宿主框架一个字都不会变，所以搬到这一层。
//!
//! ## 为什么入参是字符串而不是 `MediaStream`
//!
//! 直播与点播共用这条规则，但两者的流结构不同；而且同一条流在「挑视频
//! 地址」与「挑音频地址」时行为不一样（`disable_audio_cdn`）。按候选
//! 列表取参数，两边都能用。

use url::Url;

/// 可选的 CDN。`host` 为 `None` 的两项不是真的 CDN，是「怎么挑」的指令。
///
/// 取值与基线 `CDNService`（`lib/models/common/video/cdn_type.dart`）
/// 一一对应，**顺序即宿主下拉框的顺序**：宿主按序号传进来，不重复维护
/// 一张表。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdnService {
    /// 不挑，直接用接口给的第一个地址。
    BaseUrl,
    /// 命中镜像站时用原地址，不改 host。
    BackupUrl,
    /// 指定镜像站。
    Mirror(&'static str),
}

/// 基线那张表，按下拉框顺序。
///
/// `(标识, 展示名, host)`。展示名是中文 UI 文案，留在这里只是因为它与
/// host 必须一一对应——宿主直接用，不要再抄一份。
pub const CDN_SERVICES: &[(&str, &str, Option<&str>)] = &[
    ("baseUrl", "基础URL（不推荐）", None),
    ("backupUrl", "备用URL", None),
    (
        "ali",
        "ali（阿里云）",
        Some("upos-sz-mirrorali.bilivideo.com"),
    ),
    (
        "alib",
        "alib（阿里云）",
        Some("upos-sz-mirroralib.bilivideo.com"),
    ),
    (
        "alio1",
        "alio1（阿里云）",
        Some("upos-sz-mirroralio1.bilivideo.com"),
    ),
    (
        "cos",
        "cos（腾讯云）",
        Some("upos-sz-mirrorcos.bilivideo.com"),
    ),
    (
        "cosb",
        "cosb（腾讯云，VOD加速类型）",
        Some("upos-sz-mirrorcosb.bilivideo.com"),
    ),
    (
        "coso1",
        "coso1（腾讯云）",
        Some("upos-sz-mirrorcoso1.bilivideo.com"),
    ),
    (
        "hw",
        "hw（华为云，融合CDN）",
        Some("upos-sz-mirrorhw.bilivideo.com"),
    ),
    (
        "hwb",
        "hwb（华为云，融合CDN）",
        Some("upos-sz-mirrorhwb.bilivideo.com"),
    ),
    (
        "hwo1",
        "hwo1（华为云，融合CDN）",
        Some("upos-sz-mirrorhwo1.bilivideo.com"),
    ),
    (
        "hw_08c",
        "08c（华为云，融合CDN）",
        Some("upos-sz-mirror08c.bilivideo.com"),
    ),
    (
        "hw_08h",
        "08h（华为云，融合CDN）",
        Some("upos-sz-mirror08h.bilivideo.com"),
    ),
    (
        "hw_08ct",
        "08ct（华为云，融合CDN）",
        Some("upos-sz-mirror08ct.bilivideo.com"),
    ),
    (
        "tf_hw",
        "tf_hw（华为云）",
        Some("upos-tf-all-hw.bilivideo.com"),
    ),
    (
        "tf_tx",
        "tf_tx（腾讯云）",
        Some("upos-tf-all-tx.bilivideo.com"),
    ),
    (
        "akamai",
        "akamai（Akamai海外）",
        Some("upos-hz-mirrorakam.akamaized.net"),
    ),
    (
        "aliov",
        "aliov（阿里云海外）",
        Some("upos-sz-mirroraliov.bilivideo.com"),
    ),
    (
        "cosov",
        "cosov（腾讯云海外）",
        Some("upos-sz-mirrorcosov.bilivideo.com"),
    ),
    (
        "hwov",
        "hwov（华为云海外）",
        Some("upos-sz-mirrorhwov.bilivideo.com"),
    ),
    (
        "hk_bcache",
        "hk_bcache（Bilibili海外）",
        Some("cn-hk-eq-bcache-01.bilivideo.com"),
    ),
];

/// Akamai 的 host。它的地址自带签名，换 host 会留下一个对不上的签名。
const AKAMAI_HOST: &str = "upos-hz-mirrorakam.akamaized.net";

/// mcdn 回源要走的 tf 代理。
const PROXY_TF: &str = "proxy-tf-all-ws.bilivideo.com";

/// 默认回落的镜像站，与基线 `CDNService.ali.host` 一致。
const DEFAULT_HOST: &str = "upos-sz-mirrorali.bilivideo.com";

impl CdnService {
    /// 按标识取。认不出时回落到 `ali`——基线在 host 为空时也是这么兜的。
    #[must_use]
    pub fn from_key(key: &str) -> Self {
        match key {
            "baseUrl" => Self::BaseUrl,
            "backupUrl" => Self::BackupUrl,
            _ => CDN_SERVICES
                .iter()
                .find(|(k, _, _)| *k == key)
                .and_then(|(_, _, host)| *host)
                .map_or(Self::Mirror(DEFAULT_HOST), Self::Mirror),
        }
    }

    fn host(self) -> Option<&'static str> {
        match self {
            Self::BaseUrl | Self::BackupUrl => None,
            Self::Mirror(host) => Some(host),
        }
    }
}

/// 从候选地址里挑一个真正要播的。
///
/// [`urls`] 的顺序即协议顺序：主地址在前，备用地址依次在后。
/// [`disable_audio_cdn`] 为真且这次是音频时，不改 host——音频换 CDN 的
/// 收益小，踩到签名问题的代价大。
///
/// 候选为空时返回空串：基线在这里会抛（`urls.first` 对空集合），而空串
/// 让调用方的「拿不到地址」分支统一成一处判断。
#[must_use]
pub fn pick_cdn_url(
    urls: &[String],
    service: CdnService,
    is_audio: bool,
    disable_audio_cdn: bool,
) -> String {
    let Some(first) = urls.first() else {
        return String::new();
    };
    let keep_origin = is_audio && disable_audio_cdn;

    if service == CdnService::BaseUrl {
        return first.clone();
    }

    if service == CdnService::Mirror(AKAMAI_HOST) && !keep_origin {
        // Akamai 的地址自带签名（hdnts 之类），把别家的 host 换成它只会
        // 留下一个对不上的签名，换来 403。原样用接口给的那条。
        return urls
            .iter()
            .find(|u| host_of(u).as_deref() == Some(AKAMAI_HOST))
            .unwrap_or(first)
            .clone();
    }

    let mut mcdn_tf: Option<&String> = None;
    let mut mcdn_upgcxcode: Option<&String> = None;
    let mut last: &String = first;

    for url in urls {
        last = url;

        if is_mirror(url) {
            let parsed = Url::parse(url).ok();
            let is_mcdn = parsed
                .as_ref()
                .is_some_and(|u| u.query_pairs().any(|(k, v)| k == "os" && v == "mcdn"));
            if is_mcdn {
                // upos-sz-mirrorcoso1.bilivideo.com os=mcdn
                mcdn_upgcxcode = Some(url);
            } else {
                if service == CdnService::BackupUrl || keep_origin {
                    return url.clone();
                }
                let replaced = match (parsed, service.host()) {
                    (Some(mut u), Some(host)) => {
                        u.set_host(Some(host)).ok().map(|()| u.to_string())
                    }
                    _ => None,
                };
                return replaced.unwrap_or_else(|| url.clone());
            }
            continue;
        }

        if is_mcdn_tf(url) {
            mcdn_tf = Some(url);
            continue;
        }

        // upos-\w*-302.* & bcache & mcdn host 但路径是 upgcxcode
        if url.contains("/upgcxcode/") {
            mcdn_upgcxcode = Some(url);
            continue;
        }

        // 可能已废弃
        if url.contains("szbdyd.com") {
            if let Ok(mut parsed) = Url::parse(url) {
                let hostname = parsed
                    .query_pairs()
                    .find(|(k, _)| k == "xy_usource")
                    .map(|(_, v)| v.into_owned())
                    .or_else(|| service.host().map(str::to_owned));
                if let Some(hostname) = hostname
                    && parsed.set_scheme("https").is_ok()
                    && parsed.set_host(Some(&hostname)).is_ok()
                    && parsed.set_port(Some(443)).is_ok()
                {
                    return parsed.to_string();
                }
            }
            return url.clone();
        }
    }

    match (mcdn_upgcxcode, mcdn_tf) {
        (Some(url), _) => match Url::parse(url) {
            Ok(mut parsed) => {
                let host = service.host().unwrap_or(DEFAULT_HOST);
                if parsed.set_host(Some(host)).is_ok() {
                    parsed.to_string()
                } else {
                    url.clone()
                }
            }
            Err(_) => url.clone(),
        },
        (None, Some(url)) => {
            let mut proxy = Url::parse(&format!("https://{PROXY_TF}/")).unwrap_or_else(|_| {
                // 常量字面量，解析不了只可能是本地 bug；给一个不会 panic 的兜底。
                Url::parse("https://example.invalid/").unwrap_or_else(|_| unreachable!())
            });
            proxy.query_pairs_mut().append_pair("url", url);
            proxy.to_string()
        }
        (None, None) => last.clone(),
    }
}

fn host_of(url: &str) -> Option<String> {
    Url::parse(url).ok()?.host_str().map(str::to_owned)
}

/// `^https?://(?:upos-\w+-(?!302)\w+|(?:upos|proxy)-tf-[^/]+)\.(?:bilivideo|akamaized)\.(?:com|net)/upgcxcode`
///
/// **手写而不是用 `regex`**：那条正则带一个否定先行断言 `(?!302)`，
/// `regex` crate 不支持。改写成等价的「切出来再判断」比起换一个带回溯的
/// 正则引擎，依赖更少也更好读。
fn is_mirror(url: &str) -> bool {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    let Some(rest) = rest else { return false };
    let Some((authority, path)) = rest.split_once('/') else {
        return false;
    };
    if !path.starts_with("upgcxcode") {
        return false;
    }

    // 域名部分：<前缀>.(bilivideo|akamaized).(com|net)
    let Some((prefix, domain)) = authority.split_once('.') else {
        return false;
    };
    if !matches!(
        domain,
        "bilivideo.com" | "bilivideo.net" | "akamaized.com" | "akamaized.net"
    ) {
        return false;
    }

    if let Some(tail) = prefix
        .strip_prefix("upos-tf-")
        .or_else(|| prefix.strip_prefix("proxy-tf-"))
    {
        // `[^/]+`：到这里已经没有 `/` 了，非空即可
        return !tail.is_empty();
    }

    // upos-\w+-(?!302)\w+
    let Some(tail) = prefix.strip_prefix("upos-") else {
        return false;
    };
    let Some((mid, last)) = tail.split_once('-') else {
        return false;
    };
    !mid.is_empty()
        && mid.chars().all(is_word_char)
        && !last.starts_with("302")
        && !last.is_empty()
        && last.chars().all(is_word_char)
}

/// `^https?://(?:(?:(?:\d{1,3}\.){3}\d{1,3}|[^/]+\.mcdn\.bilivideo\.(?:com|cn|net))(?::\d{1,5})?/v\d/resource)`
fn is_mcdn_tf(url: &str) -> bool {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    let Some(rest) = rest else { return false };
    let Some((authority, path)) = rest.split_once('/') else {
        return false;
    };

    // /v<digit>/resource
    let Some(after_v) = path.strip_prefix('v') else {
        return false;
    };
    let mut chars = after_v.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_digit()) {
        return false;
    }
    if !chars.as_str().starts_with("/resource") {
        return false;
    }

    let host = authority.split_once(':').map_or(authority, |(h, _)| h);
    is_ipv4(host)
        || (host.ends_with(".mcdn.bilivideo.com")
            || host.ends_with(".mcdn.bilivideo.cn")
            || host.ends_with(".mcdn.bilivideo.net"))
}

fn is_ipv4(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    parts.len() == 4
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 3 && p.chars().all(|c| c.is_ascii_digit()))
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALI: CdnService = CdnService::Mirror("upos-sz-mirrorali.bilivideo.com");

    fn v(urls: &[&str]) -> Vec<String> {
        urls.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn base_url_takes_the_first_candidate() {
        let urls = v(&["https://upos-sz-mirrorcos.bilivideo.com/upgcxcode/a", "b"]);
        assert_eq!(
            pick_cdn_url(&urls, CdnService::BaseUrl, false, false),
            urls[0]
        );
    }

    #[test]
    fn mirror_host_is_replaced() {
        let urls = v(&["https://upos-sz-mirrorcos.bilivideo.com/upgcxcode/a?x=1"]);
        assert_eq!(
            pick_cdn_url(&urls, ALI, false, false),
            "https://upos-sz-mirrorali.bilivideo.com/upgcxcode/a?x=1"
        );
    }

    #[test]
    fn backup_url_keeps_the_mirror_untouched() {
        let urls = v(&["https://upos-sz-mirrorcos.bilivideo.com/upgcxcode/a"]);
        assert_eq!(
            pick_cdn_url(&urls, CdnService::BackupUrl, false, false),
            urls[0]
        );
    }

    #[test]
    fn audio_keeps_origin_when_audio_cdn_disabled() {
        let urls = v(&["https://upos-sz-mirrorcos.bilivideo.com/upgcxcode/a"]);
        assert_eq!(pick_cdn_url(&urls, ALI, true, true), urls[0]);
        // 视频不受这个开关影响
        assert_ne!(pick_cdn_url(&urls, ALI, false, true), urls[0]);
    }

    #[test]
    fn akamai_picks_its_own_signed_url() {
        let akamai = CdnService::Mirror(AKAMAI_HOST);
        let urls = v(&[
            "https://upos-sz-mirrorcos.bilivideo.com/upgcxcode/a",
            "https://upos-hz-mirrorakam.akamaized.net/upgcxcode/a?hdnts=sig",
        ]);
        assert_eq!(pick_cdn_url(&urls, akamai, false, false), urls[1]);
        // 没有 akamai 的候选时回落到第一个
        let only = v(&["https://upos-sz-mirrorcos.bilivideo.com/upgcxcode/a"]);
        assert_eq!(pick_cdn_url(&only, akamai, false, false), only[0]);
    }

    #[test]
    fn os_mcdn_mirror_is_not_used_directly() {
        // 命中镜像正则但带 os=mcdn：攒着，最后换 host
        let urls = v(&["https://upos-sz-mirrorcoso1.bilivideo.com/upgcxcode/a?os=mcdn"]);
        assert_eq!(
            pick_cdn_url(&urls, ALI, false, false),
            "https://upos-sz-mirrorali.bilivideo.com/upgcxcode/a?os=mcdn"
        );
    }

    #[test]
    fn mcdn_tf_goes_through_the_proxy() {
        let urls = v(&["https://1.2.3.4:8000/v1/resource/x.m4s"]);
        let picked = pick_cdn_url(&urls, ALI, false, false);
        assert!(
            picked.starts_with("https://proxy-tf-all-ws.bilivideo.com/?url="),
            "{picked}"
        );
        assert!(picked.contains("1.2.3.4"), "{picked}");
    }

    #[test]
    fn szbdyd_falls_back_to_its_source_host() {
        let urls = v(&["https://a.szbdyd.com/x?xy_usource=cn-hk-eq-bcache-01.bilivideo.com"]);
        let picked = pick_cdn_url(&urls, ALI, false, false);
        assert!(
            picked.starts_with("https://cn-hk-eq-bcache-01.bilivideo.com"),
            "{picked}"
        );
    }

    #[test]
    fn unknown_candidates_fall_back_to_the_last_one() {
        let urls = v(&[
            "https://whatever.example.com/x",
            "https://other.example.com/y",
        ]);
        assert_eq!(pick_cdn_url(&urls, ALI, false, false), urls[1]);
    }

    #[test]
    fn empty_candidates_give_an_empty_string() {
        assert!(pick_cdn_url(&[], ALI, false, false).is_empty());
    }

    #[test]
    fn mirror_regex_rejects_302() {
        assert!(!is_mirror(
            "https://upos-sz-302abc.bilivideo.com/upgcxcode/a"
        ));
        assert!(is_mirror(
            "https://upos-sz-mirrorali.bilivideo.com/upgcxcode/a"
        ));
        assert!(is_mirror(
            "https://proxy-tf-all-ws.bilivideo.com/upgcxcode/a"
        ));
        // 路径不是 upgcxcode
        assert!(!is_mirror(
            "https://upos-sz-mirrorali.bilivideo.com/other/a"
        ));
    }

    #[test]
    fn mcdn_tf_regex_matches_ip_and_mcdn_hosts() {
        assert!(is_mcdn_tf("http://10.0.0.1/v1/resource/x"));
        assert!(is_mcdn_tf(
            "https://cn-abc.mcdn.bilivideo.cn:4483/v1/resource/x"
        ));
        assert!(!is_mcdn_tf("https://cn-abc.mcdn.bilivideo.cn/v1/other/x"));
        assert!(!is_mcdn_tf("https://example.com/v1/resource/x"));
    }

    #[test]
    fn service_table_keys_resolve() {
        assert_eq!(CdnService::from_key("baseUrl"), CdnService::BaseUrl);
        assert_eq!(CdnService::from_key("backupUrl"), CdnService::BackupUrl);
        assert_eq!(
            CdnService::from_key("akamai"),
            CdnService::Mirror(AKAMAI_HOST)
        );
        // 认不出的回落到 ali
        assert_eq!(
            CdnService::from_key("nope"),
            CdnService::Mirror(DEFAULT_HOST)
        );
    }
}
