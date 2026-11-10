//! `/x/web-interface/suggest` 响应解析；与 Dart `SearchSuggestModel` 对齐。

use bili_domain::error::DomainError;
use bili_domain::search::{SearchSuggestion, SearchSuggestions};
use serde::Deserialize;

#[derive(Deserialize)]
struct DataDto {
    result: SuggestDto,
}

#[derive(Deserialize)]
struct SuggestDto {
    tag: Option<Vec<ItemDto>>,
}

#[derive(Deserialize)]
struct ItemDto {
    term: Option<String>,
    name: String,
}

/// 将完整业务响应解析成搜索建议。
///
/// # Errors
/// 信封失败沿用统一分类；字段类型与基线模型不符时返回 `Protocol`。
pub fn parse(body: &str) -> Result<SearchSuggestions, DomainError> {
    let data: DataDto = crate::envelope::unwrap_envelope(body)?;
    let tag = data.result.tag.map(|items| {
        items
            .into_iter()
            .map(|item| SearchSuggestion {
                term: item.term,
                text_rich: item.name,
            })
            .collect()
    });
    Ok(SearchSuggestions { tag })
}
