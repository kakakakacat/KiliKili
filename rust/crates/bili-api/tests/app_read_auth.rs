//! 手工构造参数，签名期望由独立 Python urllib/hashlib 计算（#260）。
//! 全部 token 为假值，不是线上响应或用户凭据。
#![allow(clippy::expect_used)]
use bili_api::request::PreparedRequest;
use bili_api::{dynamic, space};
use serde::Deserialize;

const KEY: &str = "dfca71928277209b";
const SEC: &str = "b5475a8825547a4fc26c7d518eaaa02e";
const TS: i64 = 1_700_000_000;

fn requests(token: Option<&str>) -> Vec<(&'static str, PreparedRequest)> {
    let mut requests = vec![(
        "space",
        space::space_request(1, Some("123"), token, KEY, SEC, TS),
    )];
    for (name, kind) in [
        ("video", space::SpaceArchiveKind::Video),
        ("charging", space::SpaceArchiveKind::Charging),
        ("season", space::SpaceArchiveKind::Season),
        ("series", space::SpaceArchiveKind::Series),
        ("bangumi", space::SpaceArchiveKind::Bangumi),
        ("comic", space::SpaceArchiveKind::Comic),
    ] {
        let video = matches!(kind, space::SpaceArchiveKind::Video);
        let series = matches!(kind, space::SpaceArchiveKind::Series);
        requests.push((
            name,
            space::space_archive_request(
                kind,
                Some(1),
                None,
                video.then_some(space::SpaceArchiveOrder::Pubdate),
                series.then_some(space::SpaceArchiveSort::Asc),
                (!video).then_some(1),
                video.then_some(0),
                matches!(kind, space::SpaceArchiveKind::Season).then_some(42),
                series.then_some(43),
                video.then_some(true),
                token,
                KEY,
                SEC,
                TS,
            ),
        ));
    }
    requests.extend([
        (
            "article",
            space::space_article_request(1, 2, 10, token, KEY, SEC, TS),
        ),
        (
            "coin",
            space::coin_like_arc_request(true, 1, 2, 20, token, KEY, SEC, TS),
        ),
        (
            "like",
            space::coin_like_arc_request(false, 1, 2, 20, token, KEY, SEC, TS),
        ),
        (
            "topic_top",
            dynamic::topic_top_request("10001", token, KEY, SEC, TS),
        ),
        (
            "topic_first",
            dynamic::topic_pub_search_request("关键词", "", 1, token, KEY, SEC, TS),
        ),
        (
            "topic_third",
            dynamic::topic_pub_search_request("关键词", "", 3, token, KEY, SEC, TS),
        ),
    ]);
    requests
}

#[derive(Deserialize)]
struct Case {
    name: String,
    path: String,
    anonymous: Vec<(String, String)>,
    authenticated: Vec<(String, String)>,
}

#[test]
fn every_app_read_request_matches_independent_anonymous_and_login_signatures() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("fixtures/app_read_auth.json"))
        .expect("fixture 必须可解析");
    assert_eq!(cases.len(), 13, "12 条路径加一条话题翻页请求");
    for token in [None, Some(""), Some("test-access-key")] {
        let requests = requests(token);
        assert_eq!(requests.len(), cases.len());
        for ((name, request), case) in requests.into_iter().zip(&cases) {
            assert_eq!(name, case.name);
            assert_eq!(request.path, case.path);
            let expected = if token.is_some_and(|value| !value.is_empty()) {
                &case.authenticated
            } else {
                &case.anonymous
            };
            assert_eq!(&request.query, expected, "{name} 签名或参数不符");
        }
    }
}

use bili_api::http::{HttpGet, HttpPost, HttpResponse, HttpTransport, RequestContext};
use bili_domain::error::{DomainError, ErrorKind};
use bili_domain::redact::Redacted;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

struct Capture {
    token: Option<&'static str>,
    count: AtomicUsize,
}

impl HttpTransport for Capture {
    async fn get(&self, request: HttpGet) -> Result<HttpResponse, DomainError> {
        assert!(
            !request
                .headers
                .iter()
                .any(|(key, _)| key.eq_ignore_ascii_case("cookie")),
            "APP 请求不能携带 Web Cookie"
        );
        assert!(
            request
                .headers
                .iter()
                .any(|(key, value)| key == "x-test-account" && value == "selected"),
            "账号头仍需保留"
        );
        let url = url::Url::parse(&request.url).expect("请求 URL 应合法");
        let query: std::collections::BTreeMap<_, _> = url.query_pairs().collect();
        assert_eq!(
            query.get("access_key").map(|value| value.as_ref()),
            self.token.filter(|value| !value.is_empty())
        );
        assert_eq!(
            query.get("ts").map(|value| value.as_ref()),
            Some("1700000000")
        );
        assert!(query.contains_key("sign"));
        self.count.fetch_add(1, Ordering::Relaxed);
        Ok(HttpResponse {
            status: 200,
            body: r#"{"code":0,"data":{}}"#.to_owned(),
        })
    }

    async fn post(&self, _: HttpPost) -> Result<HttpResponse, DomainError> {
        Err(DomainError::new(
            ErrorKind::Protocol,
            "只读 APP 回归不应发 POST",
        ))
    }
}

#[tokio::test]
async fn all_app_fetch_paths_send_token_and_signature_without_cookie() {
    let ctx = RequestContext {
        base_url: "https://app.example.invalid".to_owned(),
        cookie: Some(Redacted::new("SESSDATA=fake-web-session".to_owned())),
        csrf: None,
        headers: vec![
            ("Cookie".to_owned(), "fake-header-cookie".to_owned()),
            ("x-test-account".to_owned(), "selected".to_owned()),
        ],
        timeout: Duration::from_secs(1),
    };
    for token in [None, Some(""), Some("test-access-key")] {
        let transport = Capture {
            token,
            count: AtomicUsize::new(0),
        };
        space::fetch_space_page(&transport, &ctx, 1, None, token, KEY, SEC, TS)
            .await
            .expect("空间请求成功");
        for kind in [
            space::SpaceArchiveKind::Video,
            space::SpaceArchiveKind::Charging,
            space::SpaceArchiveKind::Season,
            space::SpaceArchiveKind::Series,
            space::SpaceArchiveKind::Bangumi,
            space::SpaceArchiveKind::Comic,
        ] {
            space::fetch_space_archive(
                &transport,
                &ctx,
                kind,
                Some(1),
                None,
                None,
                None,
                Some(1),
                None,
                None,
                None,
                None,
                token,
                KEY,
                SEC,
                TS,
            )
            .await
            .expect("列表请求成功");
        }
        space::fetch_space_article(&transport, &ctx, 1, 1, 10, token, KEY, SEC, TS)
            .await
            .expect("专栏请求成功");
        for coin in [true, false] {
            space::fetch_coin_like_arc(&transport, &ctx, coin, 1, 1, 20, token, KEY, SEC, TS)
                .await
                .expect("投币/点赞请求成功");
        }
        dynamic::fetch_topic_top(&transport, &ctx, "1", token, KEY, SEC, TS)
            .await
            .expect("话题顶部请求成功");
        for page in [1, 3] {
            dynamic::fetch_topic_pub_search(
                &transport,
                &ctx,
                "关键词",
                "",
                page,
                token,
                KEY,
                SEC,
                TS,
            )
            .await
            .expect("话题搜索请求成功");
        }
        assert_eq!(transport.count.load(Ordering::Relaxed), 13);
    }
    assert!(ctx.cookie.is_some(), "复用的 Web 上下文本身不应被修改");
}
