//! 评论正文的分词（S2 续单 / issue #179）。
//!
//! 对应基线 `lib/pages/video/reply/widgets/reply_item_grpc.dart` 里的
//! `_buildMessage`：把表情占位符、`@昵称`、`#话题#`、链接字面量、
//! `{vote:N}`、`(h:)mm:ss` 拼成一条正则，`splitMapJoin` 之后逐段判定
//! 「这一段是什么」。
//!
//! ## 为什么在领域层
//!
//! 按 AGENTS.md §2 的判据：换掉宿主框架之后，「哪一段是表情、哪一段是
//! 可点的链接、点下去跳哪」这些判定原样成立，要重写的只有 `InlineSpan`
//! 的拼装。判定因此属于 Rust，拼装留在宿主。
//!
//! 跟着一起下来的还有几条同类决定：备选词的拼接顺序（正则是
//! leftmost-first，顺序即优先级）、同一条链接只渲染一次、时间戳的有效性
//! 判据、`[笔记]` 的位置与可点性。它们单独看都像「界面细节」，但没有一条
//! 是协议规定的，而两端又必须一致。
//!
//! ## 顺手修掉的一处不确定性
//!
//! 基线的备选词顺序取自 `content.emotes.keys` 等的迭代序。#176 把这些
//! 字段投影成 `HashMap` 之后，Dart 侧拿到的顺序**已经不稳定**了：前缀
//! 互相遮蔽的两个 `@` 名（`@张三` 与 `@张三丰`）哪个先匹配，取决于一次
//! 哈希迭代。本模块按 [`candidate_order`] 定死：**同组内长度降序，同长按
//! 字典序**。长的在前，短的就不会把长的遮掉。
//!
//! 这不是「顺手优化」。它是协议没规定、两端必须一致的决定，正是 §2 要求
//! 下沉的那一类；留在宿主的话，换一个宿主就会换一种随机顺序。
//!
//! ## `\d` 不能照抄
//!
//! 基线的时间戳与投票用的是 Dart `RegExp` 的 `\d`，**只认 ASCII**；Rust
//! `regex` 的 `\d` 认 Unicode 全集，全角数字 `１` 会命中。两端语义不同的
//! 构造正是 [`crate::regex_compat`] 要拒绝的那一类，这里是我们自己写的
//! 正则、改得了，所以直接写成 `[0-9]`。
//!
//! ## 分词失败为什么不报错
//!
//! 备选词来自评论内容，条数没有上界，正则理论上会撞到 `regex` 的体积
//! 上限。此时 [`segment`] 退化成「整条消息一段纯文本」而不是报错：它不是
//! 过滤规则，失败不会让被屏蔽的内容漏出来，而把整条评论变成一个错误提示
//! 显然更糟。这条与 ADR-010 决策二的区别就在这里——那条管的是过滤。

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::reply_grpc::{ReplyEmote, ReplyNote, ReplyUrl, ReplyVote};

/// 通用 URL 的识别式。基线 `Constants.urlRegex`，逐字相同。
///
/// 不含 `\d` / `\b` / `{n,m}` 这类两端语义不同的构造（见
/// [`crate::regex_compat::divergent_construct`]），可以直译。
pub const URL_PATTERN: &str = r"https?://[-A-Za-z0-9+&@#/%?=~_|!:,.;]+[-A-Za-z0-9+&@#/%=~_|]";

/// 时间戳。基线 `(?:\d+[:：])?\d+[:：]\d+`，`\d` 换成 `[0-9]`。
const TIMESTAMP_PATTERN: &str = r"(?:[0-9]+[:：])?[0-9]+[:：][0-9]+";

/// 投票占位。基线 `\{vote:\d+?\}`，惰性量词照抄（此处与贪婪等价，但没有
/// 理由在转写时改写它）。
const VOTE_PATTERN: &str = r"\{vote:[0-9]+?\}";

static URL_RE: LazyLock<Regex> = LazyLock::new(|| compile(URL_PATTERN));
static TIMESTAMP_RE: LazyLock<Regex> = LazyLock::new(|| compile(&format!("^{TIMESTAMP_PATTERN}$")));
static VOTE_RE: LazyLock<Regex> = LazyLock::new(|| compile(&format!("^{VOTE_PATTERN}$")));
/// 基线 `RegExp(r'^(av|bv)', caseSensitive: false)`。
static ARCHIVE_RE: LazyLock<Regex> = LazyLock::new(|| compile("(?i)^(?:av|bv)"));
/// 基线 `RegExp(r'^cv(\d+)|/read/cv(\d+)|note-app/view\?cvid=(\d+)',
/// caseSensitive: false)`。`^` 只管第一个分支，照抄。
static ARTICLE_RE: LazyLock<Regex> =
    LazyLock::new(|| compile(r"(?i)^cv([0-9]+)|/read/cv([0-9]+)|note-app/view\?cvid=([0-9]+)"));

/// 编译一条**本模块自己写死**的正则。
///
/// 这些式子不来自用户配置，编不过就是本文件写错了，属于编程错误而不是
/// 运行时状况——所以这里 panic，而 [`segment`] 里那条由内容拼出来的正则
/// 走降级路径。
fn compile(pattern: &str) -> Regex {
    #[allow(clippy::expect_used)]
    Regex::new(pattern).expect("本模块内置的正则必须能编过")
}

/// 分词要读的那几个字段，从 `ReplyContent` + `ReplyControlInfo` 投影而来。
///
/// 形状照 [`crate::reply_filter::ReplyFilterItem`]：只带真正要读的字段。
/// 这不只是洁癖——分词在**每次 widget build** 上跑，配图与成员表跟着过
/// 一趟 FFI 是白付的钱。
#[derive(Debug, Clone, Default)]
pub struct ReplyRichTextInput {
    pub message: String,
    /// 占位文本（`[doge]`）→ 表情。
    pub emotes: HashMap<String, ReplyEmote>,
    /// 话题名，不带两侧的 `#`。
    pub topics: Vec<String>,
    /// `@` 后面的昵称 → mid。
    pub at_name_to_mid: HashMap<String, i64>,
    /// 正文里出现的字面量 → 跳转信息。
    pub urls: HashMap<String, ReplyUrl>,
    pub vote: Option<ReplyVote>,
    pub note: Option<ReplyNote>,
    /// `ReplyControlInfo::is_note`。
    pub is_note: bool,
    /// `ReplyControlInfo::is_note_v2`。
    pub is_note_v2: bool,
}

/// 宿主状态。Rust 不缓存，按调用传入（同 [`crate::reply_filter`] 的设置）。
#[derive(Debug, Clone, Copy, Default)]
pub struct ReplyRichTextSettings {
    /// 基线 `ReplyItemGrpc.enableWordRe`（`Pref.enableWordRe`）。关掉时
    /// 词条搜索链接退化成纯文本。
    pub enable_word_re: bool,
    /// 当前稿件时长（毫秒）。`None` 对应基线取不到播放器控制器的那条路
    /// （`Get.find` 抛异常 → `isValid` 保持 false），此时时间戳一律不可点。
    pub video_duration_ms: Option<i64>,
}

/// 一段正文是什么。
///
/// 为什么不是带字段的枚举：FRB 的导出面要保持语言中立（AGENTS.md §4），
/// 本仓库导出的枚举一律是无字段的，载荷放在结构的字段上。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ReplySegmentKind {
    /// 纯文本。
    Text,
    /// 表情图。
    Emote,
    /// `@某人`。
    Mention,
    /// `#话题#`。
    Topic,
    /// `{vote:N}`，正文里引用的投票。
    Vote,
    /// `(h:)mm:ss`。
    Timestamp,
    /// 可点的链接。
    Link,
    /// 笔记前缀 `[笔记] `。
    Note,
}

/// 点下去干什么。载荷统一放 [`ReplySegment::target`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ReplyTapAction {
    /// 不可点。界面按次要色画。
    None,
    /// 内置 webview 打开 `target`。
    Webview,
    /// 稿件页。`target` 是 av/bv 字面量，由宿主的 url 解析继续认。
    Archive,
    /// 专栏。`target` 是 cvid。
    Article,
    /// 图文动态。`target` 是 opus id。
    Opus,
    /// 走 app scheme 路由。`target` 是服务端给的跳转串。
    Scheme,
    /// 搜索结果页。`target` 是关键词。
    Search,
}

/// 正文里的一段。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ReplySegment {
    pub kind: ReplySegmentKind,
    /// 要画出来的文本。[`ReplySegmentKind::Emote`] 段是占位符原文（界面
    /// 画图，但长按复制要的是它）。
    pub text: String,
    /// [`ReplySegmentKind::Mention`]：被 @ 的 mid。
    pub mid: Option<i64>,
    /// [`ReplySegmentKind::Topic`]：不带 `#` 的话题名，也是搜索关键词。
    pub topic: Option<String>,
    /// [`ReplySegmentKind::Emote`]：表情本身。
    pub emote: Option<ReplyEmote>,
    /// [`ReplySegmentKind::Vote`]：投票 id 与标题。
    pub vote: Option<ReplyVote>,
    /// [`ReplySegmentKind::Timestamp`]：要跳到的秒数。`None` 表示超出
    /// 稿件时长或解析不出来，此时基线只画成纯文本（不变色、不可点）。
    pub seek_seconds: Option<i64>,
    /// [`ReplySegmentKind::Link`]：标题前的小图标。
    pub prefix_icon: Option<String>,
    /// 点击行为。非 Link / Note 段一律 [`ReplyTapAction::None`]——它们的
    /// 跳转由各自的字段表达。
    pub action: ReplyTapAction,
    /// [`Self::action`] 的载荷。
    pub target: String,
}

impl ReplySegment {
    fn text(text: impl Into<String>) -> Self {
        Self {
            kind: ReplySegmentKind::Text,
            text: text.into(),
            mid: None,
            topic: None,
            emote: None,
            vote: None,
            seek_seconds: None,
            prefix_icon: None,
            action: ReplyTapAction::None,
            target: String::new(),
        }
    }

    fn of(kind: ReplySegmentKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            ..Self::text(text)
        }
    }
}

/// 备选词在正则里的排序：长度降序，同长按字典序。
///
/// 长度降序是**判定的一部分**：正则的 `|` 是 leftmost-first，短的排在前
/// 就会把与它同前缀的长词吃掉（`@张三` 吃掉 `@张三丰`）。字典序只是为了
/// 把剩下的不确定性也去掉——同长的两个词不可能互相遮蔽，但顺序稳定才能
/// 让 fixture 对得上。
///
/// 长度按**字节**比较。同一组里的候选词都是同一来源的文本，字节长度与
/// 字符长度在「谁是谁的前缀」这件事上给出相同的答案（UTF-8 前缀即字符
/// 前缀），用字节只是省一次遍历。
fn candidate_order(mut items: Vec<String>) -> Vec<String> {
    items.sort_unstable_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    items
}

/// 按基线的顺序拼出分词用的正则源串。
///
/// 组间顺序照抄基线：表情 → 话题 → `@` 名 → 链接字面量 → 时间戳 → 投票
/// → 通用 URL。组内顺序见 [`candidate_order`]。
fn build_pattern(input: &ReplyRichTextInput) -> String {
    fn push_all(parts: &mut Vec<String>, items: Vec<String>) {
        parts.extend(candidate_order(items).iter().map(|s| regex::escape(s)));
    }

    let mut parts: Vec<String> = Vec::new();
    push_all(&mut parts, input.emotes.keys().cloned().collect());
    push_all(
        &mut parts,
        input.topics.iter().map(|t| format!("#{t}#")).collect(),
    );
    push_all(
        &mut parts,
        input
            .at_name_to_mid
            .keys()
            .map(|name| format!("@{name}"))
            .collect(),
    );
    push_all(&mut parts, input.urls.keys().cloned().collect());

    parts.push(TIMESTAMP_PATTERN.to_owned());
    parts.push(VOTE_PATTERN.to_owned());
    parts.push(URL_PATTERN.to_owned());
    parts.join("|")
}

/// 把一条正文切成段。
///
/// 结果里**不含空的纯文本段**：基线 `splitMapJoin` 的 `onNonMatch` 在两个
/// 相邻匹配之间会给一个空串，`TextSpan(text: '')` 画出来什么都没有。这里
/// 直接不发，省掉宿主一次无谓的 span。
#[must_use]
pub fn segment(input: &ReplyRichTextInput, settings: &ReplyRichTextSettings) -> Vec<ReplySegment> {
    let Ok(pattern) = Regex::new(&build_pattern(input)) else {
        // 见模块文档：分词不是过滤，失败降级成纯文本。
        return vec![ReplySegment::text(input.message.clone())];
    };

    // `front` 收的是要挪到正文最前面的那些（笔记链接）。基线每命中一条
    // 就 `insertAll(0, ...)`，于是多条笔记链接的相对顺序是**倒序**——
    // 照抄，不在转写时顺手「修正」。
    let mut front: Vec<ReplySegment> = Vec::new();
    let mut body: Vec<ReplySegment> = Vec::new();
    let mut matched_urls: Vec<&str> = Vec::new();
    let mut has_note = false;
    let mut cursor = 0usize;

    for m in pattern.find_iter(&input.message) {
        if m.start() > cursor {
            body.push(ReplySegment::text(&input.message[cursor..m.start()]));
        }
        cursor = m.end();
        classify(
            m.as_str(),
            input,
            settings,
            &mut front,
            &mut body,
            &mut matched_urls,
            &mut has_note,
        );
    }
    if cursor < input.message.len() {
        body.push(ReplySegment::text(&input.message[cursor..]));
    }

    // 正文里没有笔记链接，但这条评论被标成笔记：补一个前缀。两个跳转
    // 目标都没有时只画一个不可点的「[笔记]」。
    if !has_note && input.is_note && input.is_note_v2 {
        let mut seg = ReplySegment::of(ReplySegmentKind::Note, "[笔记] ");
        if let Some(note) = input.note.as_ref() {
            if let Some(url) = note.click_url.as_ref() {
                seg.action = ReplyTapAction::Scheme;
                seg.target.clone_from(url);
            } else if let Some(id) = note.opus_id {
                seg.action = ReplyTapAction::Opus;
                seg.target = id.to_string();
            }
        }
        front.insert(0, seg);
    }

    front.extend(body);
    front
}

/// 判定一个匹配是什么。分支顺序照抄基线，**与它由哪一个备选词匹配出来
/// 无关**：一个长得像时间戳的表情占位符，基线也是先走表情那一支。
fn classify<'a>(
    matched: &'a str,
    input: &ReplyRichTextInput,
    settings: &ReplyRichTextSettings,
    front: &mut Vec<ReplySegment>,
    body: &mut Vec<ReplySegment>,
    matched_urls: &mut Vec<&'a str>,
    has_note: &mut bool,
) {
    if let Some(emote) = input.emotes.get(matched) {
        // 表情本身不可点：基线把 `jump_url` 交给长按弹出的 tooltip，
        // span 上没有手势识别器。载荷在 `emote` 里，宿主自己读。
        let mut seg = ReplySegment::of(ReplySegmentKind::Emote, matched);
        seg.emote = Some(emote.clone());
        body.push(seg);
        return;
    }

    if let Some(name) = matched.strip_prefix('@')
        && let Some(mid) = input.at_name_to_mid.get(name)
    {
        let mut seg = ReplySegment::of(ReplySegmentKind::Mention, matched);
        seg.mid = Some(*mid);
        body.push(seg);
        return;
    }

    if VOTE_RE.is_match(matched)
        && let Some(vote) = input.vote.as_ref()
    {
        let mut seg = ReplySegment::of(ReplySegmentKind::Vote, matched);
        seg.vote = Some(vote.clone());
        body.push(seg);
        return;
    }

    if TIMESTAMP_RE.is_match(matched) {
        // 基线先把全角冒号换掉，再拿换过的串去解析**并显示**。
        let normalized = matched.replace('：', ":");
        let seconds = parse_duration(&normalized).filter(|s| match settings.video_duration_ms {
            // 基线：`parseDuration(matchStr) * 1000 <= ctr.data.timeLength!`。
            Some(ms) => s.checked_mul(1000).is_some_and(|v| v <= ms),
            None => false,
        });
        // 两侧补空格（基线对可点的那些做的事）留给宿主：那是排版，
        // 不是判定。这里只给归一化后的文本与秒数。
        let mut seg = ReplySegment::of(ReplySegmentKind::Timestamp, normalized);
        seg.seek_seconds = seconds;
        body.push(seg);
        return;
    }

    if let Some(url) = input.urls.get(matched)
        && !matched_urls.contains(&matched)
    {
        matched_urls.push(matched);
        add_url(matched, url, settings, front, body, has_note);
        return;
    }

    // 同一条链接第二次出现时落到这里：基线不再给它标题与图标，只要它
    // 长得像 URL 就当成一条普通的可点链接。照抄。
    if let Some(topic) = inner(matched)
        && input.topics.iter().any(|t| t == topic)
    {
        let mut seg = ReplySegment::of(ReplySegmentKind::Topic, matched);
        seg.topic = Some(topic.to_owned());
        seg.action = ReplyTapAction::Search;
        seg.target = topic.to_owned();
        body.push(seg);
        return;
    }

    if URL_RE.is_match(matched) {
        let mut seg = ReplySegment::of(ReplySegmentKind::Link, matched);
        seg.action = ReplyTapAction::Webview;
        seg.target = matched.to_owned();
        body.push(seg);
        return;
    }

    body.push(ReplySegment::text(matched));
}

/// 一条有标题的链接。
fn add_url(
    literal: &str,
    url: &ReplyUrl,
    settings: &ReplyRichTextSettings,
    front: &mut Vec<ReplySegment>,
    body: &mut Vec<ReplySegment>,
    has_note: &mut bool,
) {
    // 关掉词条搜索时退化成纯文本。注意这一支在 `has_note` 之前——基线
    // 的 early return 也在那之前，于是一条「词条搜索的笔记链接」不会把
    // 正文末尾的 `[笔记]` 前缀顶掉。
    if url.is_word_search && !settings.enable_word_re {
        body.push(ReplySegment::text(literal));
        return;
    }

    let is_note = url.is_note;
    let (action, target) = tap_action(literal, url);
    let mut seg = ReplySegment::of(
        ReplySegmentKind::Link,
        if is_note {
            "[笔记] "
        } else {
            url.title.as_str()
        },
    );
    seg.action = action;
    seg.target = target;
    if !is_note {
        seg.prefix_icon.clone_from(&url.prefix_icon);
    }

    if is_note {
        *has_note = true;
        front.insert(0, seg);
    } else {
        body.push(seg);
    }
}

/// 一条链接点下去跳哪。
///
/// 服务端没给 scheme 时，基线按字面量自己认：`av`/`bv` 开头当稿件，能抠出
/// cvid 的当专栏，其余丢给 webview。
fn tap_action(literal: &str, url: &ReplyUrl) -> (ReplyTapAction, String) {
    if url.app_url_schema.is_empty() {
        if ARCHIVE_RE.is_match(literal) {
            return (ReplyTapAction::Archive, literal.to_owned());
        }
        if let Some(cvid) = article_id(literal) {
            return (ReplyTapAction::Article, cvid);
        }
        return (ReplyTapAction::Webview, literal.to_owned());
    }
    if url.is_word_search {
        return (ReplyTapAction::Search, url.title.clone());
    }
    (ReplyTapAction::Webview, literal.to_owned())
}

/// 从链接字面量里抠出 cvid。三个分支里第一个命中的那组。
fn article_id(literal: &str) -> Option<String> {
    let caps = ARTICLE_RE.captures(literal)?;
    (1..=3)
        .find_map(|i| caps.get(i))
        .map(|m| m.as_str().to_owned())
}

/// 去掉首尾各一个字符。基线 `String.substring1`。
///
/// 基线按 UTF-16 码元取（`substring(1, length - 1)`），这里按字符。两者
/// 只在「匹配出来的整段恰好是一个非 BMP 字符」时不同：那时基线得到空串，
/// 本函数给 `None`，而空串不可能是一个话题名，判定结果仍然一致。
fn inner(text: &str) -> Option<&str> {
    let mut chars = text.char_indices();
    let (_, first) = chars.next()?;
    let (last_start, _) = chars.next_back()?;
    Some(&text[first.len_utf8()..last_start])
}

/// 基线 `DurationUtils.parseDuration`：按 `:` 切开，从右往左每段乘 60 的
/// 幂次。
///
/// 解析不出来（空段、超出 `i64`）时返回 `None`。基线此时 `int.parse` 抛
/// `FormatException`，被 `isValid` 外面那个 `try/catch` 吃掉，结果同样是
/// 「不可点」——两端行为一致，只是一边靠异常一边靠 `Option`。
fn parse_duration(text: &str) -> Option<i64> {
    let mut total: i64 = 0;
    let mut unit: i64 = 1;
    for part in text.rsplit(':') {
        let value: i64 = part.parse().ok()?;
        total = total.checked_add(value.checked_mul(unit)?)?;
        unit = unit.checked_mul(60)?;
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(message: &str) -> ReplyRichTextInput {
        ReplyRichTextInput {
            message: message.to_owned(),
            ..Default::default()
        }
    }

    fn kinds(segments: &[ReplySegment]) -> Vec<ReplySegmentKind> {
        segments.iter().map(|s| s.kind).collect()
    }

    #[test]
    fn plain_message_is_one_text_segment() {
        let segs = segment(
            &input("没有任何可点的东西"),
            &ReplyRichTextSettings::default(),
        );
        assert_eq!(kinds(&segs), vec![ReplySegmentKind::Text]);
        assert_eq!(segs[0].text, "没有任何可点的东西");
    }

    #[test]
    fn longer_mention_is_not_shadowed_by_its_prefix() {
        // 这条是 `candidate_order` 存在的理由：两个名字互为前缀时，短的
        // 排在前面会把长的吃掉，而基线里谁在前取决于一次哈希迭代。
        let mut i = input("@张三丰 你好");
        i.at_name_to_mid.insert("张三".to_owned(), 1);
        i.at_name_to_mid.insert("张三丰".to_owned(), 2);

        let segs = segment(&i, &ReplyRichTextSettings::default());
        assert_eq!(segs[0].kind, ReplySegmentKind::Mention);
        assert_eq!(segs[0].text, "@张三丰");
        assert_eq!(segs[0].mid, Some(2));
    }

    #[test]
    fn timestamp_beyond_duration_is_not_tappable() {
        let settings = ReplyRichTextSettings {
            enable_word_re: false,
            video_duration_ms: Some(60_000),
        };
        let segs = segment(&input("00:30 和 02:00"), &settings);
        let stamps: Vec<_> = segs
            .iter()
            .filter(|s| s.kind == ReplySegmentKind::Timestamp)
            .collect();
        assert_eq!(stamps.len(), 2);
        assert_eq!(stamps[0].seek_seconds, Some(30));
        assert_eq!(stamps[0].text, "00:30");
        assert_eq!(stamps[1].seek_seconds, None, "超出时长就不可点");
        assert_eq!(stamps[1].text, "02:00");
    }

    #[test]
    fn full_width_colon_is_normalized_before_parsing() {
        let settings = ReplyRichTextSettings {
            enable_word_re: false,
            video_duration_ms: Some(600_000),
        };
        let segs = segment(&input("01：30"), &settings);
        assert_eq!(segs[0].kind, ReplySegmentKind::Timestamp);
        assert_eq!(segs[0].seek_seconds, Some(90));
        assert_eq!(segs[0].text, "01:30", "全角冒号归一化后再发给宿主");
    }

    #[test]
    fn full_width_digits_are_not_timestamps() {
        // Rust `regex` 的 `\d` 会认全角数字，Dart 的不会。写成 `[0-9]`
        // 就是为了这一条。
        let segs = segment(&input("０１:３０"), &ReplyRichTextSettings::default());
        assert_eq!(kinds(&segs), vec![ReplySegmentKind::Text]);
    }

    #[test]
    fn duplicated_url_literal_loses_its_title() {
        let mut i = input("https://b23.tv/x 再贴一次 https://b23.tv/x");
        i.urls.insert(
            "https://b23.tv/x".to_owned(),
            ReplyUrl {
                title: "标题".to_owned(),
                ..Default::default()
            },
        );
        let segs = segment(&i, &ReplyRichTextSettings::default());
        let links: Vec<_> = segs
            .iter()
            .filter(|s| s.kind == ReplySegmentKind::Link)
            .collect();
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].text, "标题");
        assert_eq!(links[1].text, "https://b23.tv/x", "第二次只当普通链接");
    }

    #[test]
    fn word_search_link_degrades_to_text_when_disabled() {
        let mut i = input("看看 词条 吧");
        i.urls.insert(
            "词条".to_owned(),
            ReplyUrl {
                title: "词条".to_owned(),
                app_url_schema: "bilibili://search".to_owned(),
                is_word_search: true,
                ..Default::default()
            },
        );

        let off = segment(&i, &ReplyRichTextSettings::default());
        assert_eq!(kinds(&off), vec![ReplySegmentKind::Text; 3]);

        let on = segment(
            &i,
            &ReplyRichTextSettings {
                enable_word_re: true,
                video_duration_ms: None,
            },
        );
        assert_eq!(on[1].kind, ReplySegmentKind::Link);
        assert_eq!(on[1].action, ReplyTapAction::Search);
        assert_eq!(on[1].target, "词条");
    }

    #[test]
    fn note_link_moves_to_the_front() {
        let mut i = input("正文 cv114514 结尾");
        i.urls.insert(
            "cv114514".to_owned(),
            ReplyUrl {
                title: "一篇笔记".to_owned(),
                is_note: true,
                ..Default::default()
            },
        );
        let segs = segment(&i, &ReplyRichTextSettings::default());
        assert_eq!(segs[0].kind, ReplySegmentKind::Link);
        assert_eq!(segs[0].text, "[笔记] ");
        assert_eq!(segs[0].action, ReplyTapAction::Article);
        assert_eq!(segs[0].target, "114514");
    }

    #[test]
    fn note_prefix_is_added_when_the_body_has_no_note_link() {
        let mut i = input("正文");
        i.is_note = true;
        i.is_note_v2 = true;
        i.note = Some(ReplyNote {
            click_url: None,
            opus_id: Some(42),
        });
        let segs = segment(&i, &ReplyRichTextSettings::default());
        assert_eq!(
            kinds(&segs),
            vec![ReplySegmentKind::Note, ReplySegmentKind::Text]
        );
        assert_eq!(segs[0].action, ReplyTapAction::Opus);
        assert_eq!(segs[0].target, "42");
    }

    #[test]
    fn note_prefix_without_a_target_is_not_tappable() {
        let mut i = input("正文");
        i.is_note = true;
        i.is_note_v2 = true;
        let segs = segment(&i, &ReplyRichTextSettings::default());
        assert_eq!(segs[0].kind, ReplySegmentKind::Note);
        assert_eq!(segs[0].action, ReplyTapAction::None);
    }

    #[test]
    fn vote_without_payload_falls_through_to_text() {
        let segs = segment(&input("{vote:1}"), &ReplyRichTextSettings::default());
        assert_eq!(kinds(&segs), vec![ReplySegmentKind::Text]);
    }

    #[test]
    fn topic_is_recognized_and_searchable() {
        let mut i = input("聊聊 #原神# 吧");
        i.topics.push("原神".to_owned());
        let segs = segment(&i, &ReplyRichTextSettings::default());
        assert_eq!(segs[1].kind, ReplySegmentKind::Topic);
        assert_eq!(segs[1].topic.as_deref(), Some("原神"));
        assert_eq!(segs[1].action, ReplyTapAction::Search);
    }

    #[test]
    fn duration_parsing_matches_the_baseline() {
        assert_eq!(parse_duration("90"), Some(90));
        assert_eq!(parse_duration("01:30"), Some(90));
        assert_eq!(parse_duration("1:00:00"), Some(3600));
        assert_eq!(
            parse_duration("01:"),
            None,
            "空段在基线里是 FormatException"
        );
    }

    #[test]
    fn inner_strips_one_char_on_each_side() {
        assert_eq!(inner("#原神#"), Some("原神"));
        assert_eq!(inner("ab"), Some(""));
        assert_eq!(inner("a"), None);
    }
}
