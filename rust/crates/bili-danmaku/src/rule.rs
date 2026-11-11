//! 弹幕屏蔽规则：关键词、正则、UID。
//!
//! 基线实现是 `lib/models/user/danmaku_rule.dart` 的 `RuleFilter.remove`：
//! UID 命中、关键词子串命中、正则命中，三者任一即移除。
//!
//! 关键词与 UID 是精确可移植的（子串包含、集合命中）。**正则不是**：
//! 基线用 Dart `RegExp`（ECMAScript 语法，UTF-16 码元语义），Rust 用
//! `regex`（无回溯，Unicode 码点语义）。两者的交集很大，但不是全集，
//! 而差集里的行为是**静默**的——同一条弹幕一边过滤了、一边没过滤，
//! 用户看到的是「我明明拉黑了，它还在飘」。
//!
//! 因此本模块的原则是：**只接受两端语义相同的子集，其余一律拒绝**，
//! 由调用方整条路径降级回 Dart（issue #42、ADR-010）。拒绝是安全的，
//! 它只是让这条弹幕链路回到基线；放行才是危险的。
//!
//! 被拒绝的构造见 [`divergent_construct`]。

use std::collections::HashSet;

use bili_domain::danmaku::DanmakuElem;
use bili_domain::error::{DomainError, ErrorKind};
use regex::{Regex, RegexBuilder};

/// 编译后的屏蔽规则。
#[derive(Debug, Clone, Default)]
pub struct FilterRules {
    keywords: Vec<String>,
    regexes: Vec<Regex>,
    uids: HashSet<String>,
}

impl FilterRules {
    /// 按用户配置编译规则。
    ///
    /// `patterns` 是**已去掉两端斜杠**的正则源串，与基线存进 Hive 的
    /// 形态一致（`RuleFilterAdapter.write` 存的是 `RegExp.pattern`，
    /// `fromRuleTypeEntries` 在构造 `RegExp` 前就剥掉了 `/.../`）。
    ///
    /// # Errors
    /// 任何一条正则编不过、或用到两端语义不同的构造时返回
    /// [`ErrorKind::Unsupported`]。调用方**必须**据此整条降级回 Dart，
    /// 而不是丢掉这一条规则继续——少过滤一条弹幕没有任何提示。
    pub fn compile(
        keywords: Vec<String>,
        patterns: &[String],
        uids: impl IntoIterator<Item = String>,
    ) -> Result<Self, DomainError> {
        let mut regexes = Vec::with_capacity(patterns.len());
        for (index, pattern) in patterns.iter().enumerate() {
            if let Some(construct) = divergent_construct(pattern) {
                return Err(DomainError::new(
                    ErrorKind::Unsupported,
                    // 不带正则原文：它来自用户配置，可能含有个人信息。
                    format!("第 {index} 条正则使用了两端语义不同的构造 {construct}"),
                ));
            }
            let regex = RegexBuilder::new(pattern)
                // 基线固定 caseSensitive: false（两处构造点都是）。
                .case_insensitive(true)
                .build()
                .map_err(|e| {
                    DomainError::new(
                        ErrorKind::Unsupported,
                        format!("第 {index} 条正则 Rust 引擎编不过: {}", first_line(&e)),
                    )
                })?;
            regexes.push(regex);
        }

        Ok(Self {
            keywords,
            regexes,
            uids: uids.into_iter().collect(),
        })
    }

    /// 规则条数。基线的 `filters.count`：为 0 时整个过滤步骤跳过。
    #[must_use]
    pub fn len(&self) -> usize {
        self.keywords.len() + self.regexes.len() + self.uids.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 这条弹幕是否应被移除。字段与判定顺序同基线 `RuleFilter.remove`。
    #[must_use]
    pub fn removes(&self, elem: &DanmakuElem) -> bool {
        self.uids.contains(&elem.mid_hash)
            || self.keywords.iter().any(|k| elem.content.contains(k))
            || self.regexes.iter().any(|r| r.is_match(&elem.content))
    }
}

/// 正则里是否含有「两端都能编过、但语义不同」的构造；有则返回它。
///
/// 编不过的构造（lookaround、反向引用等）不在此列——它们由 `regex`
/// 自己报错，拒绝路径相同。这里要挡住的是**悄悄不同**的那些：
///
/// - `\d` `\w` `\s` 及其大写：Dart 只认 ASCII，Rust 认 Unicode 全集
///   （全角数字 `１` 命中 Rust 的 `\d`，不命中 Dart 的）。
/// - `\b` `\B`：同理，单词边界的「单词」定义不同。
/// - `{n,m}`：计数单位不同。Dart 数 UTF-16 码元，Rust 数码点；一个
///   非 BMP 字符（emoji）在前者是 2、在后者是 1。
///
/// 扫描时按转义配对前进，因此 `\\d`（字面反斜杠加 d）不会误判；字符类
/// 内部同样检查，`[\d]` 一样被拒。宁可多拒（降级回基线，结果仍然正确），
/// 不可漏判。
#[must_use]
pub fn divergent_construct(pattern: &str) -> Option<&'static str> {
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('d') => return Some(r"\d"),
                Some('D') => return Some(r"\D"),
                Some('w') => return Some(r"\w"),
                Some('W') => return Some(r"\W"),
                Some('s') => return Some(r"\s"),
                Some('S') => return Some(r"\S"),
                Some('b') => return Some(r"\b"),
                Some('B') => return Some(r"\B"),
                // 其余转义（含 `\\`）整体跳过，下一轮从它后面继续。
                _ => {}
            },
            '{' => return Some("{n,m}"),
            _ => {}
        }
    }
    None
}

/// 取错误的首行。`regex` 的错误是多行的（带指位记号），塞进一行日志里
/// 很难读，而首行已经说清了原因。
fn first_line(error: &regex::Error) -> String {
    error
        .to_string()
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned()
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{FilterRules, divergent_construct};
    use bili_domain::danmaku::DanmakuElem;
    use bili_domain::error::ErrorKind;

    fn elem(content: &str, mid_hash: &str) -> DanmakuElem {
        DanmakuElem {
            content: content.to_owned(),
            mid_hash: mid_hash.to_owned(),
            ..DanmakuElem::default()
        }
    }

    #[test]
    fn empty_rules_remove_nothing() {
        let rules = FilterRules::default();
        assert!(rules.is_empty());
        assert!(!rules.removes(&elem("前方高能", "a1b2")));
    }

    #[test]
    fn keyword_is_a_case_sensitive_substring() {
        // 基线用 String.contains，不折叠大小写——只有正则是大小写不敏感的。
        let rules = FilterRules::compile(vec!["AD".to_owned()], &[], []).expect("可编译");
        assert!(rules.removes(&elem("这是AD内容", "x")));
        assert!(!rules.removes(&elem("这是ad内容", "x")));
    }

    #[test]
    fn uid_matches_the_mid_hash() {
        let rules = FilterRules::compile(vec![], &[], ["deadbeef".to_owned()]).expect("可编译");
        assert!(rules.removes(&elem("无关内容", "deadbeef")));
        assert!(!rules.removes(&elem("无关内容", "cafebabe")));
    }

    #[test]
    fn regex_is_case_insensitive_like_the_baseline() {
        let rules = FilterRules::compile(vec![], &["^spam".to_owned()], []).expect("可编译");
        assert!(rules.removes(&elem("SPAM 来了", "x")));
        assert!(!rules.removes(&elem("不是 spam 开头", "x")));
    }

    #[test]
    fn rule_count_sums_all_three_kinds() {
        let rules = FilterRules::compile(
            vec!["a".to_owned(), "b".to_owned()],
            &["c".to_owned()],
            ["d".to_owned(), "d".to_owned()],
        )
        .expect("可编译");
        // UID 是集合，重复项只算一个——与基线的 Set 一致。
        assert_eq!(rules.len(), 4);
    }

    #[test]
    fn lookahead_is_rejected_instead_of_silently_dropped() {
        let err = FilterRules::compile(vec![], &["foo(?=bar)".to_owned()], [])
            .expect_err("Rust 不支持 lookahead");
        assert_eq!(err.kind(), ErrorKind::Unsupported);
    }

    #[test]
    fn backreference_is_rejected() {
        let err = FilterRules::compile(vec![], &[r"(a)\1".to_owned()], [])
            .expect_err("Rust 不支持反向引用");
        assert_eq!(err.kind(), ErrorKind::Unsupported);
    }

    #[test]
    fn unicode_shorthands_are_rejected() {
        for pattern in [r"\d+", r"[\w]", r"\bfoo\b", r"\S"] {
            let err = FilterRules::compile(vec![], &[pattern.to_owned()], [])
                .expect_err("语义不同，必须拒绝");
            assert_eq!(err.kind(), ErrorKind::Unsupported, "{pattern}");
        }
    }

    #[test]
    fn counted_repetition_is_rejected() {
        // `{n,m}` 的计数单位两端不同：一个 emoji 在 Dart 里是 2，在 Rust 里是 1。
        let err = FilterRules::compile(vec![], &["^.{10,}$".to_owned()], [])
            .expect_err("计数单位不同，必须拒绝");
        assert_eq!(err.kind(), ErrorKind::Unsupported);
    }

    #[test]
    fn escaped_backslash_is_not_mistaken_for_a_shorthand() {
        // `\\d` 是「字面反斜杠 + 字母 d」，不是 `\d`。
        assert_eq!(divergent_construct(r"\\d"), None);
        assert_eq!(divergent_construct(r"\\\d"), Some(r"\d"));
        let rules = FilterRules::compile(vec![], &[r"\\d".to_owned()], []).expect("可编译");
        assert!(rules.removes(&elem(r"反斜杠\d 本身", "x")));
    }

    #[test]
    fn plain_regex_without_divergence_is_accepted() {
        assert_eq!(divergent_construct("^(前|后)方.*能$"), None);
        assert_eq!(divergent_construct(r"\."), None);
        assert_eq!(divergent_construct("[0-9]+"), None);
    }

    #[test]
    fn empty_pattern_matches_everything_like_the_baseline() {
        // Dart 的 RegExp('') 对任何字符串 hasMatch 为真。
        let rules = FilterRules::compile(vec![], &[String::new()], []).expect("可编译");
        assert!(rules.removes(&elem("任何内容", "x")));
    }
}
