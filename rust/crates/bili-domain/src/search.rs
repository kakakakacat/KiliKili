//! 搜索业务实体。建议项保留服务端富文本原样，由 Flutter 负责展示。

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchSuggestions {
    pub tag: Option<Vec<SearchSuggestion>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchSuggestion {
    pub term: Option<String>,
    #[serde(rename = "name")]
    pub text_rich: String,
}
