//! `/x/web-interface/wbi/view` 的 DTO 与到领域实体的映射。
//!
//! DTO 一律 `Option` 且 `#[serde(default)]`：接口随时可能少给字段，
//! 而一个字段缺失不该让整个详情页打不开。真正的结构性变更由
//! [`crate::envelope`] 的 Protocol 错误暴露。
//!
//! 映射中的每一处「怪异」都来自基线 Dart 模型
//! （`lib/models_new/video/video_detail/`），不是设计选择；保留它们是为了
//! 让 Rust 与 Dart 对同一份响应产出逐字段相同的结果，否则 shadow mode
//! 的比对会被我们自己的「改进」淹没。每处都在注释里标了出处与原因。

use bili_domain::error::DomainError;
use bili_domain::video::{
    DescSegment, Dimension, EpisodeArc, SeasonEpisode, SeasonSection, Staff, UgcSeason, UserBrief,
    VideoDetail, VideoPart, VideoStat,
};
use serde::Deserialize;

/// 解析一次视频详情响应。
///
/// 入参是**整个响应体**（含信封），因为 code 非 0 时根本没有 data，
/// 把拆信封留给调用方只会让每个接口各写一遍。
///
/// # Errors
/// 见 [`crate::envelope::unwrap_envelope`]。
pub fn parse(body: &str) -> Result<VideoDetail, DomainError> {
    let dto: VideoDetailDto = crate::envelope::unwrap_envelope(body)?;
    Ok(dto.into_domain())
}

/// 宽松整数：服务端对 mid 之类的字段混用过数字与字符串。
///
/// 对应基线 `safeToInt`（`lib/utils/parse_int.dart`）：数字取整、
/// 字符串尝试解析、其余为 null。**不可**简化成 `as i64`——一旦遇到
/// 字符串形式，serde 会整条记录解析失败，而基线只会让该字段为 null。
fn flexible_i64(v: Option<&serde_json::Value>) -> Option<i64> {
    match v? {
        serde_json::Value::Number(n) => n.as_i64().or_else(|| {
            // Dart 的 num.toInt() 对浮点是截断而非四舍五入。
            #[allow(clippy::cast_possible_truncation)]
            n.as_f64().map(|f| f.trunc() as i64)
        }),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

// 全字段 Option + `#[serde(default)]`：缺字段时为 None。注意是
// `Option<Vec<_>>` 而非 `Vec<_>`——「接口没返回 pages」与「返回了空
// pages」在 UI 上是两件事（显不显示分 P 面板）。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct VideoDetailDto {
    bvid: Option<String>,
    aid: Option<i64>,
    videos: Option<i64>,
    copyright: Option<i64>,
    pic: Option<String>,
    title: Option<String>,
    pubdate: Option<i64>,
    ctime: Option<i64>,
    desc: Option<String>,
    desc_v2: Option<Vec<DescV2Dto>>,
    duration: Option<i64>,
    rights: Option<RightsDto>,
    owner: Option<OwnerDto>,
    stat: Option<StatDto>,
    argue_info: Option<ArgueInfoDto>,
    cid: Option<i64>,
    dimension: Option<DimensionDto>,
    season_id: Option<i64>,
    is_upower_exclusive: Option<bool>,
    pages: Option<Vec<PartDto>>,
    ugc_season: Option<UgcSeasonDto>,
    staff: Option<Vec<StaffDto>>,
    redirect_url: Option<String>,
}

impl VideoDetailDto {
    fn into_domain(self) -> VideoDetail {
        VideoDetail {
            bvid: self.bvid,
            aid: self.aid,
            videos: self.videos,
            copyright: self.copyright,
            pic: self.pic,
            title: self.title,
            pubdate: self.pubdate,
            ctime: self.ctime,
            desc: self.desc,
            desc_v2: self
                .desc_v2
                .map(|v| v.into_iter().map(DescV2Dto::into_domain).collect()),
            duration: self.duration,
            is_stein_gate: self.rights.and_then(|r| r.is_stein_gate),
            owner: self.owner.map(OwnerDto::into_domain),
            stat: self.stat.map(StatDto::into_domain),
            argue_msg: self.argue_info.and_then(|a| a.argue_msg),
            cid: self.cid,
            dimension: self.dimension.map(DimensionDto::into_domain),
            season_id: self.season_id,
            is_upower_exclusive: self.is_upower_exclusive,
            pages: self
                .pages
                .map(|v| v.into_iter().map(PartDto::into_domain).collect()),
            ugc_season: self.ugc_season.map(UgcSeasonDto::into_domain),
            staff: self
                .staff
                .map(|v| v.into_iter().map(StaffDto::into_domain).collect()),
            // 基线用 `nonNullOrEmptyString`：空串与缺失同义。
            redirect_url: self.redirect_url.filter(|s| !s.is_empty()),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct DescV2Dto {
    raw_text: Option<String>,
    r#type: Option<i64>,
    biz_id: Option<i64>,
}

impl DescV2Dto {
    fn into_domain(self) -> DescSegment {
        DescSegment {
            raw_text: self.raw_text,
            r#type: self.r#type,
            biz_id: self.biz_id,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RightsDto {
    is_stein_gate: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OwnerDto {
    mid: Option<serde_json::Value>,
    name: Option<String>,
    face: Option<String>,
}

impl OwnerDto {
    fn into_domain(self) -> UserBrief {
        UserBrief {
            mid: flexible_i64(self.mid.as_ref()),
            name: self.name,
            face: self.face,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StatDto {
    view: Option<i64>,
    danmaku: Option<i64>,
    reply: Option<i64>,
    favorite: Option<i64>,
    coin: Option<serde_json::Value>,
    share: Option<i64>,
    like: Option<i64>,
}

impl StatDto {
    fn into_domain(self) -> VideoStat {
        VideoStat {
            view: self.view,
            danmaku: self.danmaku,
            reply: self.reply,
            // 基线对这三个字段用了 `?? 0`：它们参与点赞/投币/收藏的
            // 数值展示与增减，null 会让 UI 显示空白。
            favorite: self.favorite.unwrap_or(0),
            coin: flexible_i64(self.coin.as_ref()).unwrap_or(0),
            share: self.share,
            like: self.like.unwrap_or(0),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ArgueInfoDto {
    argue_msg: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct DimensionDto {
    width: Option<i64>,
    height: Option<i64>,
    rotate: Option<i64>,
}

impl DimensionDto {
    fn into_domain(self) -> Dimension {
        // 竖屏视频服务端给的是横向尺寸 + rotate=1。基线在模型里就地
        // 交换（`dimension.dart`），下游全部依赖「拿到的就是显示尺寸」。
        if self.rotate == Some(1) {
            Dimension {
                width: self.height,
                height: self.width,
            }
        } else {
            Dimension {
                width: self.width,
                height: self.height,
            }
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PartDto {
    cid: Option<i64>,
    page: Option<i64>,
    from: Option<String>,
    part: Option<String>,
    duration: Option<i64>,
    vid: Option<String>,
    dimension: Option<DimensionDto>,
    ctime: Option<i64>,
    first_frame: Option<String>,
    badge: Option<String>,
}

impl PartDto {
    fn into_domain(self) -> VideoPart {
        VideoPart {
            cid: self.cid,
            page: self.page,
            from: self.from,
            part: self.part,
            duration: self.duration,
            vid: self.vid,
            dimension: self.dimension.map(DimensionDto::into_domain),
            ctime: self.ctime,
            first_frame: self.first_frame,
            badge: self.badge,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UgcSeasonDto {
    id: Option<i64>,
    title: Option<String>,
    cover: Option<String>,
    mid: Option<i64>,
    sections: Option<Vec<SectionDto>>,
}

impl UgcSeasonDto {
    fn into_domain(self) -> UgcSeason {
        UgcSeason {
            id: self.id,
            title: self.title,
            cover: self.cover,
            mid: self.mid,
            sections: self
                .sections
                .map(|v| v.into_iter().map(SectionDto::into_domain).collect()),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SectionDto {
    season_id: Option<i64>,
    id: Option<i64>,
    title: Option<String>,
    r#type: Option<i64>,
    episodes: Option<Vec<EpisodeDto>>,
}

impl SectionDto {
    fn into_domain(self) -> SeasonSection {
        SeasonSection {
            season_id: self.season_id,
            id: self.id,
            title: self.title,
            r#type: self.r#type,
            episodes: self
                .episodes
                .map(|v| v.into_iter().map(EpisodeDto::into_domain).collect()),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct EpisodeDto {
    season_id: Option<i64>,
    section_id: Option<i64>,
    id: Option<i64>,
    aid: Option<i64>,
    bvid: Option<String>,
    cid: Option<i64>,
    title: Option<String>,
    attribute: Option<i64>,
    badge: Option<String>,
    arc: Option<ArcDto>,
    page: Option<PartDto>,
    pages: Option<Vec<PartDto>>,
}

impl EpisodeDto {
    fn into_domain(self) -> SeasonEpisode {
        SeasonEpisode {
            season_id: self.season_id,
            section_id: self.section_id,
            id: self.id,
            aid: self.aid,
            bvid: self.bvid,
            cid: self.cid,
            title: self.title,
            attribute: self.attribute,
            badge: self.badge,
            arc: self.arc.map(ArcDto::into_domain),
            page: self.page.map(PartDto::into_domain),
            pages: self
                .pages
                .map(|v| v.into_iter().map(PartDto::into_domain).collect()),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ArcDto {
    aid: Option<i64>,
    pic: Option<String>,
    title: Option<String>,
    pubdate: Option<i64>,
    duration: Option<i64>,
    author: Option<OwnerDto>,
    stat: Option<StatDto>,
    dimension: Option<DimensionDto>,
}

impl ArcDto {
    fn into_domain(self) -> EpisodeArc {
        EpisodeArc {
            aid: self.aid,
            pic: self.pic,
            title: self.title,
            pubdate: self.pubdate,
            duration: self.duration,
            author: self.author.map(OwnerDto::into_domain),
            stat: self.stat.map(StatDto::into_domain),
            dimension: self.dimension.map(DimensionDto::into_domain),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StaffDto {
    mid: Option<serde_json::Value>,
    title: Option<String>,
    name: Option<String>,
    face: Option<String>,
    vip: Option<VipDto>,
    official: Option<OfficialDto>,
}

impl StaffDto {
    fn into_domain(self) -> Staff {
        let vip = self.vip.unwrap_or_default();
        let official = self.official.unwrap_or_default();
        Staff {
            mid: flexible_i64(self.mid.as_ref()),
            title: self.title,
            name: self.name,
            face: self.face,
            // 基线 `Vip.fromJson` 兼容两套键名（web 与 app 端字段不同）。
            vip_type: vip.r#type.or(vip.vip_type),
            vip_status: vip.status.or(vip.vip_status).unwrap_or(0),
            official_type: official.r#type,
            official_desc: official.desc,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct VipDto {
    r#type: Option<i64>,
    #[serde(rename = "vipType")]
    vip_type: Option<i64>,
    status: Option<i64>,
    #[serde(rename = "vipStatus")]
    vip_status: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OfficialDto {
    r#type: Option<i64>,
    desc: Option<String>,
}

#[cfg(test)]
// 测试里 expect 是正确做法：前提不成立时就该崩掉并指出原因。
#[allow(clippy::expect_used)]
mod tests {
    use bili_domain::error::ErrorKind;

    use super::*;

    #[test]
    fn rejects_non_zero_code_with_classified_kind() {
        let err = parse(r#"{"code":-404,"message":"啥都木有","data":null}"#)
            .expect_err("非 0 code 必须是错误");
        assert_eq!(err.kind(), ErrorKind::Remote, "未知码按可重试处理");
        assert!(err.detail().contains("-404"), "原始码必须保留供诊断");

        let auth = parse(r#"{"code":-403,"message":"权限不足","data":null}"#)
            .expect_err("非 0 code 必须是错误");
        assert_eq!(auth.kind(), ErrorKind::Auth);
    }

    #[test]
    fn malformed_body_is_a_protocol_error() {
        let err = parse("<html>502</html>").expect_err("非 JSON 必须失败");
        assert_eq!(
            err.kind(),
            ErrorKind::Protocol,
            "协议层错误要与业务失败区分，前者意味着我们对接口的理解过时了"
        );
    }

    #[test]
    fn missing_data_on_success_is_a_protocol_error() {
        let err = parse(r#"{"code":0,"message":"0"}"#).expect_err("成功却无 data 必须失败");
        assert_eq!(err.kind(), ErrorKind::Protocol);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        // 接口加字段是常态，不该让解析失败。
        let v = parse(r#"{"code":0,"data":{"bvid":"BV1","brand_new_field":{"x":1}}}"#)
            .expect("新增字段必须被忽略");
        assert_eq!(v.bvid.as_deref(), Some("BV1"));
    }

    #[test]
    fn rotated_dimension_is_swapped() {
        let v = parse(r#"{"code":0,"data":{"dimension":{"width":1920,"height":1080,"rotate":1}}}"#)
            .expect("解析应成功");
        let d = v.dimension.expect("dimension 应存在");
        assert_eq!((d.width, d.height), (Some(1080), Some(1920)));
        assert!(d.is_vertical());
    }

    #[test]
    fn empty_redirect_url_becomes_none() {
        let v = parse(r#"{"code":0,"data":{"redirect_url":""}}"#).expect("解析应成功");
        assert_eq!(v.redirect_url, None, "空串会让调用方跳转到空地址");
    }

    #[test]
    fn string_mid_is_accepted() {
        let v = parse(r#"{"code":0,"data":{"owner":{"mid":"12345"}}}"#).expect("解析应成功");
        assert_eq!(v.owner.expect("owner 应存在").mid, Some(12345));
    }

    #[test]
    fn missing_counters_default_to_zero() {
        let v = parse(r#"{"code":0,"data":{"stat":{"view":10}}}"#).expect("解析应成功");
        let s = v.stat.expect("stat 应存在");
        assert_eq!((s.favorite, s.coin, s.like), (0, 0, 0));
        assert_eq!(s.view, Some(10));
        assert_eq!(s.danmaku, None, "缺失与 0 不同义的字段必须保持缺失");
    }

    #[test]
    fn absent_lists_stay_none() {
        let v = parse(r#"{"code":0,"data":{"bvid":"BV1"}}"#).expect("解析应成功");
        assert_eq!(v.pages, None, "「没返回分 P」不等于「分 P 列表为空」");
        assert_eq!(v.staff, None);
    }
}
