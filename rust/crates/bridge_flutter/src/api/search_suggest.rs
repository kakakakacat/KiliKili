//! 搜索建议解析桥接。只读、单次响应；失败交还 Dart 原解析路径。

#[derive(Debug, Clone)]
pub struct SearchSuggestParse {
    pub json: Option<String>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
}

#[must_use]
pub fn parse_search_suggest(body: String) -> SearchSuggestParse {
    match bili_api::search_suggest::parse(&body) {
        Ok(suggestions) => match serde_json::to_string(&suggestions) {
            Ok(json) => SearchSuggestParse {
                json: Some(json),
                error_kind: None,
                error_detail: None,
            },
            Err(e) => SearchSuggestParse {
                json: None,
                error_kind: Some("protocol".to_owned()),
                error_detail: Some(format!("搜索建议序列化失败: {e}")),
            },
        },
        Err(e) => SearchSuggestParse {
            json: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
        },
    }
}
