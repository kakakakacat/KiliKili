//! 请求构造：路径 + 已签名的查询参数。
//!
//! 产出是一个**纯数据**的 [`PreparedRequest`]，由调用方（Stage 1 是
//! Dart 的 Dio）去发。这样请求的形状可以被 fixture 锁死，而不需要起
//! 网络或 mock 一整个 HTTP 客户端。

use bili_domain::video::VideoType;

use crate::http::Idempotency;
use bili_protocol::risk::{self, RiskTokens};
use bili_protocol::wbi;

/// 接口路径常量。与基线 `lib/http/api.dart` 保持一致。
pub mod path {
    /// 视频详情（WBI 签名版）。
    pub const VIDEO_DETAIL: &str = "/x/web-interface/wbi/view";
    /// 取流：UGC。
    pub const PLAY_URL_UGC: &str = "/x/player/wbi/playurl";
    /// 取流：PGC/PUGV 主接口。
    pub const PLAY_URL_OGV: &str = "/ogv/player/playview";
    /// 取流：PGC 降级接口。
    pub const PLAY_URL_PGC: &str = "/pgc/player/web/v2/playurl";
    /// 取流：PUGV 降级接口。
    pub const PLAY_URL_PUGV: &str = "/pugv/player/web/playurl";
    /// 导航信息。WBI 的 `img_key`/`sub_key` 由此而来。
    pub const NAV: &str = "/x/web-interface/nav";
}

/// 待发送的请求。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedRequest {
    pub path: &'static str,
    /// 查询参数，**已按 key 升序**且含 `wts`/`w_rid`。
    ///
    /// 用有序列表而非 Map：签名覆盖的是序列化后的字符串，顺序是签名
    /// 的一部分，换成 Map 等于把这个约束交给调用方的实现细节。
    pub query: Vec<(String, String)>,
}

/// 构造视频详情请求。
///
/// `mixin_key` 由 [`bili_protocol::wbi::mixin_key`] 从 nav 接口的
/// key 推导；`wts` 由调用方注入（Unix 秒），使本函数保持纯函数，可被
/// golden fixture 固定。
///
/// 返回 `None` 表示 `bvid` 为空——没有 bvid 的详情请求必然失败，与其
/// 发出去换一个业务错误码，不如在此处就拒绝。
#[must_use]
pub fn video_detail(bvid: &str, mixin_key: &str, wts: i64) -> Option<PreparedRequest> {
    if bvid.is_empty() {
        return None;
    }

    let mut query = vec![
        ("bvid".to_owned(), bvid.to_owned()),
        ("wts".to_owned(), wts.to_string()),
    ];
    query.sort_by(|a, b| a.0.cmp(&b.0));

    let w_rid = wbi::sign(&query, mixin_key);
    query.push(("w_rid".to_owned(), w_rid));

    Some(PreparedRequest {
        path: path::VIDEO_DETAIL,
        query,
    })
}

/// playurl 请求的业务输入。
///
/// 与基线 `VideoHttp.videoUrl` 的参数一一对应。风控参数、固定常量与
/// 签名不在这里——它们由 [`play_url`] 按协议补齐，调用方不该有机会
/// 忘掉其中某一项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayUrlParams {
    pub avid: Option<i64>,
    pub bvid: Option<String>,
    pub ep_id: Option<i64>,
    pub season_id: Option<i64>,
    pub cid: i64,
    /// 清晰度代码。
    pub qn: i64,
    pub video_type: VideoType,
    /// 未登录试看 1080P。
    pub try_look: bool,
    /// 音量均衡。
    pub voice_balance: bool,
    /// 多语言音轨（如 PGC 的国语/日语），基线 `cur_language`。
    pub language: Option<String>,
    /// 走降级接口。
    ///
    /// 基线只在 PGC 返回 `arc.is_drm == true` 时重试一次（见
    /// `lib/http/video.dart`）：DRM 内容在主接口拿不到可播放流，降级接口
    /// 能给出非 DRM 的那一份。降级时整组 `biz_type`/`scene`/`from_client`
    /// /`drm_tech_type`/`app_id` 都要去掉——它们是主接口（playview）的
    /// 入参，降级接口不认。
    pub fallback: bool,
}

impl PlayUrlParams {
    /// 最小构造：UGC 只需 bvid + cid + qn。其余字段取基线默认值。
    #[must_use]
    pub fn ugc(bvid: impl Into<String>, cid: i64, qn: i64) -> Self {
        Self {
            avid: None,
            bvid: Some(bvid.into()),
            ep_id: None,
            season_id: None,
            cid,
            qn,
            video_type: VideoType::Ugc,
            try_look: false,
            voice_balance: false,
            language: None,
            fallback: false,
        }
    }

    /// 本次请求该用哪个接口路径。
    #[must_use]
    pub const fn api_path(&self) -> &'static str {
        match (self.video_type, self.fallback) {
            // UGC 的主接口与降级接口是同一个：基线的 fallbackApi 对 ugc
            // 填的就是 ugcUrl，降级路径实际只对 PGC/PUGV 有意义。
            (VideoType::Ugc, _) => path::PLAY_URL_UGC,
            (VideoType::Pgc | VideoType::Pugv, false) => path::PLAY_URL_OGV,
            (VideoType::Pgc, true) => path::PLAY_URL_PGC,
            (VideoType::Pugv, true) => path::PLAY_URL_PUGV,
        }
    }

    /// `biz_type` 的取值。UGC 没有这个参数。
    #[must_use]
    const fn biz_type(&self) -> Option<&'static str> {
        match self.video_type {
            VideoType::Ugc => None,
            VideoType::Pgc => Some("ogv"),
            VideoType::Pugv => Some("pugv"),
        }
    }

    /// 参数是否足以构成一个有意义的请求。
    ///
    /// 不是多余的防御：缺标识符时服务端返回的是普通业务错误码，调用链
    /// 上要回溯到「少传了一个 id」非常费劲，不如在构造处就拒绝。
    fn identifies_a_video(&self) -> bool {
        if self.cid <= 0 {
            return false;
        }
        match self.video_type {
            VideoType::Ugc => {
                self.avid.is_some() || self.bvid.as_deref().is_some_and(|s| !s.is_empty())
            }
            VideoType::Pgc | VideoType::Pugv => self.ep_id.is_some() || self.season_id.is_some(),
        }
    }
}

/// 构造 playurl 请求。
///
/// 纯函数：随机风控 token（`tokens`）与时间（`wts`）都由调用方注入，
/// 因此整个查询串连同签名可以被 golden fixture 逐项锁死
/// （`tests/playurl_golden.rs`）。若在这里就地取随机数与系统时间，
/// 请求的形状就只能靠肉眼比对。
///
/// 返回 `None` 表示参数不足以标识一个视频，见
/// [`PlayUrlParams::identifies_a_video`]。
#[must_use]
pub fn play_url(
    params: &PlayUrlParams,
    tokens: &RiskTokens,
    mixin_key: &str,
    wts: i64,
) -> Option<PreparedRequest> {
    if !params.identifies_a_video() {
        return None;
    }

    let mut query: Vec<(String, String)> = Vec::with_capacity(20);
    let mut put = |k: &str, v: String| query.push((k.to_owned(), v));

    if let Some(avid) = params.avid {
        put("avid", avid.to_string());
    }
    if let Some(bvid) = &params.bvid {
        put("bvid", bvid.clone());
    }
    if let Some(ep_id) = params.ep_id {
        put("ep_id", ep_id.to_string());
    }
    if let Some(season_id) = params.season_id {
        put("season_id", season_id.to_string());
    }

    put("cid", params.cid.to_string());
    put("qn", params.qn.to_string());
    // fnval 4048：请求服务端返回全部可用格式（DASH + 各编码 + HDR/杜比）。
    // 基线注释为「获取所有格式的视频」，是一组位标志的并，不是魔数。
    put("fnval", "4048".to_owned());
    put("fourk", "1".to_owned());
    put("fnver", "0".to_owned());
    put("voice_balance", i32::from(params.voice_balance).to_string());
    put("gaia_source", risk::GAIA_SOURCE.to_owned());
    // Dart 的 `true.toString()` 是 "true"，签名覆盖的也是这个字面量，
    // 不能写成 1。
    put("isGaiaAvoided", "true".to_owned());
    put("web_location", risk::WEB_LOCATION.to_string());

    if !params.fallback
        && let Some(biz_type) = params.biz_type()
    {
        put("biz_type", biz_type.to_owned());
        put("scene", "normal".to_owned());
        put("from_client", "BROWSER".to_owned());
        put("drm_tech_type", "0".to_owned());
        put("app_id", "100".to_owned());
    }

    if params.try_look {
        put("try_look", "1".to_owned());
    }

    put("dm_img_list", risk::DM_IMG_LIST.to_owned());
    put("dm_img_str", tokens.dm_img_str.clone());
    put("dm_cover_img_str", tokens.dm_cover_img_str.clone());
    put("dm_img_inter", risk::DM_IMG_INTER.to_owned());

    if let Some(language) = &params.language {
        put("cur_language", language.clone());
    }

    put("wts", wts.to_string());

    query.sort_by(|a, b| a.0.cmp(&b.0));
    let w_rid = wbi::sign(&query, mixin_key);
    query.push(("w_rid".to_owned(), w_rid));

    Some(PreparedRequest {
        path: params.api_path(),
        query,
    })
}

/// 待发送的**写**请求。
///
/// 与 [`PreparedRequest`] 分开两个类型，而不是给它加一个 `Option<body>`：
/// 写请求必须带 [`Idempotency`]，分开之后「忘了定级」在类型上就不成立
/// （ADR-013 决策二）。
///
/// **csrf 不在这里。** 它由 [`crate::client::send_write`] 从
/// [`crate::http::RequestContext`] 统一补进表单，是全仓库唯一的注入点
/// （ADR-013 决策四）。因此本结构是**纯业务参数**，可以原样写进请求快照
/// 而不含任何凭据——这正是 ADR-013 决策六要的那份对照物。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedWrite {
    pub path: &'static str,
    /// 表单字段。顺序即发送顺序，被请求快照钉住。
    pub form: Vec<(String, String)>,
    /// 这条写操作重发安全不安全。
    pub idempotency: Idempotency,
}

impl PreparedWrite {
    /// 拼成 `k=v&k=v` 形式。与 [`PreparedRequest::query_string`] 一样，
    /// 仅用于测试断言。
    #[must_use]
    pub fn form_string(&self) -> String {
        self.form
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&")
    }
}

impl PreparedRequest {
    /// 拼成 `k=v&k=v` 形式。仅用于调试与测试断言；真正发送时由传输层
    /// 自行编码，避免这里的编码口径与传输层不一致。
    #[must_use]
    pub fn query_string(&self) -> String {
        self.query
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&")
    }
}

#[cfg(test)]
// 测试里 expect 是正确做法：前提不成立时就该崩掉并指出原因。
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // 与 bili-protocol 的 fixture 同源的占位 key（非真实凭据）。
    const MIXIN_KEY: &str = "ea1db124af3c7062474693fa704f4ff8";

    #[test]
    fn signs_and_appends_w_rid() {
        let req = super::video_detail("BV1L94y1H7CV", MIXIN_KEY, 1_702_204_169)
            .expect("非空 bvid 必须能构造请求");
        assert_eq!(req.path, path::VIDEO_DETAIL);

        let keys: Vec<&str> = req.query.iter().map(|(k, _)| k.as_str()).collect();
        // w_rid 必须在最后：它是对前面所有参数的签名，混进排序里会让
        // 「签名覆盖了什么」变得不可一眼看清。
        assert_eq!(keys, ["bvid", "wts", "w_rid"]);
    }

    #[test]
    fn signature_is_deterministic_for_fixed_wts() {
        let a = super::video_detail("BV1L94y1H7CV", MIXIN_KEY, 1_702_204_169);
        let b = super::video_detail("BV1L94y1H7CV", MIXIN_KEY, 1_702_204_169);
        assert_eq!(a, b, "相同输入必须产出相同签名，否则无法被 fixture 锁死");
    }

    #[test]
    fn different_wts_changes_signature() {
        let a = super::video_detail("BV1L94y1H7CV", MIXIN_KEY, 1_702_204_169);
        let b = super::video_detail("BV1L94y1H7CV", MIXIN_KEY, 1_702_204_170);
        assert_ne!(a, b, "wts 参与签名，变化必须反映到 w_rid");
    }

    #[test]
    fn empty_bvid_is_rejected() {
        assert!(super::video_detail("", MIXIN_KEY, 1).is_none());
    }
}
