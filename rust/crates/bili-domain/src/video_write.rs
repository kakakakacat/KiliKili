//! 视频页写操作的领域实体（S4 / issue #54）。
//!
//! 这一片的写操作**绝大多数没有载荷**——`code == 0` 就是全部结论，
//! 用 `bridge_flutter::api::write::WriteResult` 表达即可。只有三条有：
//! 两种一键三连各回一份「现在是什么状态」，追番的加/删/改各回一句
//! 用来弹 toast 的文案。
//!
//! toast 那三条不在这里：一个 `String` 不值得包一层领域类型，它也确实
//! 只是一句服务端文案。三连的两份状态则必须有类型——它们回来之后会直接
//! 决定界面上三个按钮的亮灭。
//!
//! ## 字段为什么全是 `Option`
//!
//! 与其余切片同理（ADR-011）：服务端少回一个字段不该让整次操作判失败。
//! 三连尤其如此——它是复合操作，**部分成功是正常结果**，`like` 成了而
//! `coin` 没成的响应里，没成的那个可能根本不出现。

/// UGC 一键三连的结果。
///
/// 三个布尔各自表示那一项**这次是否生效**，`multiply` 是实际投出的币数。
/// 基线把它们直接喂给界面的三个按钮，没有任何合并判断——也就是说，
/// 「三连」在协议上从来不是一个原子操作，而是三件事的回执。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UgcTriple {
    pub like: Option<bool>,
    pub coin: Option<bool>,
    pub fav: Option<bool>,
    pub multiply: Option<i64>,
}

/// PGC（番剧/影视）一键三连的结果。
///
/// 与 UGC 不是同一组字段，也不是同一种类型：这边的 `like` / `favorite`
/// / `follow` 是**数字**而非布尔，`coin` 是本次投出的币数。基线的
/// `PgcTriple.fromJson` 原样接下来，界面按真假值判断，这里照搬——把它们
/// 规整成 bool 等于替服务端决定「几算成功」。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PgcTriple {
    /// 本次投出的币数。
    pub coin: Option<i64>,
    /// 投币后该稿件的总币数。
    pub coin_number: Option<i64>,
    pub favorite: Option<i64>,
    /// 关注的 UP 主 mid。
    pub fmid: Option<i64>,
    pub follow: Option<i64>,
    pub like: Option<i64>,
    /// 是否已关注。
    pub relation: Option<bool>,
}
