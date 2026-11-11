//! 用户配置的正则：哪些能跨到 Rust，哪些必须拒绝。
//!
//! 基线用 Dart `RegExp`（ECMAScript 语法、UTF-16 码元语义），Rust 用
//! `regex`（无回溯、Unicode 码点语义）。两者交集很大，差集里的行为却是
//! **静默**的：同一条内容一边过滤了、一边没过滤，用户看到的是「我明明
//! 屏蔽了，它还在」。
//!
//! 因此凡是来自用户配置的正则，都只接受两端语义相同的子集，其余一律
//! 拒绝（[`ErrorKind::Unsupported`]），由调用方决定怎么交代。
//!
//! 本模块原先是 `bili-danmaku::rule` 的私有实现（ADR-010 / issue #42）。
//! S3 的推荐与分区屏蔽词是同一类用户正则，判定必须只有一处定义——两处
//! 各自演化的话，「哪些构造算分歧」会在两条路径上悄悄分叉。

use crate::error::{DomainError, ErrorKind};
use regex::{Regex, RegexBuilder};

/// 按基线的构造方式编译一条用户正则（大小写不敏感）。
///
/// `label` 用于错误文案，指明是哪一项配置出的问题；**不要**把正则原文
/// 放进去，它来自用户配置，可能含有个人信息。
///
/// # Errors
/// 用到两端语义不同的构造（见 [`divergent_construct`]），或 Rust 引擎
/// 编不过（lookaround、反向引用等）时返回 [`ErrorKind::Unsupported`]。
pub fn compile_user_pattern(pattern: &str, label: &str) -> Result<Regex, DomainError> {
    if let Some(construct) = divergent_construct(pattern) {
        return Err(DomainError::new(
            ErrorKind::Unsupported,
            format!("{label}使用了两端语义不同的构造 {construct}"),
        ));
    }
    RegexBuilder::new(pattern)
        // 基线的构造点一律 caseSensitive: false。
        .case_insensitive(true)
        .build()
        .map_err(|e| {
            DomainError::new(
                ErrorKind::Unsupported,
                format!("{label} Rust 引擎编不过: {}", first_line(&e)),
            )
        })
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
/// 内部同样检查，`[\d]` 一样被拒。宁可多拒，不可漏判。
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
#[must_use]
pub fn first_line(error: &regex::Error) -> String {
    error
        .to_string()
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned()
}
