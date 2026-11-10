//! 视频详情解析的 Dart 导出。
//!
//! ## 为什么导出的是「解析」而不是「请求」
//!
//! Stage 1 的分工是 Dart 发请求、Rust 解析（理由见 `bili_api` 的 crate
//! 文档）。shadow mode 因此比对的是**同一份响应字节**在两种实现下的
//! 规范化结果：差异必然出自解析本身，不会混入网络抖动、Cookie 状态或
//! 风控返回的不同内容；也不会对只读接口重复发一次请求——RFC-0001 §5.3
//! 要求 shadow mode「禁止对写操作重复执行」，对只读接口重复请求同样是
//! 平白加一倍流量并徒增风控风险。
//!
//! ## 为什么返回 JSON 字符串而不是结构体
//!
//! 详情实体有二十余个嵌套类型。把它们全部镜像过 FFI，只为了让 Dart 拿
//! 去做一次比对，是把 Phase 2 就会删掉的桥接层写厚；而 JSON 字符串恰好
//! 就是比对所需的形式。等 Rust 真正接管详情页数据（P1-03），显示路径
//! 需要类型化的实体时，再在此处导出结构体。

use bili_api::video_detail;

/// 解析结果。
///
/// 不用 `Result`：FRB 把 Err 映射成 Dart 异常，而调用方在 shadow mode
/// 下需要的恰恰是「失败也不要打断主流程」——异常会逼每个调用点写
/// try/catch，漏写一处就会把一个纯比对功能变成线上崩溃。
#[derive(Debug, Clone)]
pub struct VideoDetailParse {
    /// 规范化后的 JSON。失败时为 `None`。
    pub json: Option<String>,
    /// 失败大类：`network|remote|protocol|auth|storage|player|cancelled`。
    /// 成功时为 `None`。
    pub error_kind: Option<String>,
    /// 失败详情，供诊断与上报。**不含请求参数与凭据。**
    pub error_detail: Option<String>,
}

/// 解析 `/x/web-interface/wbi/view` 的响应体。
///
/// `body` 是**整个响应体**（含 `code`/`message` 信封）；信封里的业务失败
/// 同样经此函数分类，调用方无需先行拆包。
#[must_use]
pub fn parse_video_detail(body: String) -> VideoDetailParse {
    match video_detail::parse(&body) {
        Ok(detail) => match serde_json::to_string(&detail) {
            Ok(json) => VideoDetailParse {
                json: Some(json),
                error_kind: None,
                error_detail: None,
            },
            // 实体序列化失败只可能源于本地 bug（实体里没有 Map<非字符串>
            // 之类的结构），但不值得为此 panic 掉宿主 App。
            Err(e) => VideoDetailParse {
                json: None,
                error_kind: Some("protocol".to_owned()),
                error_detail: Some(format!("实体序列化失败: {e}")),
            },
        },
        Err(e) => VideoDetailParse {
            json: None,
            error_kind: Some(e.kind().to_string()),
            error_detail: Some(e.detail().to_owned()),
        },
    }
}
