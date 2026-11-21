//! 弹幕数据源：golden fixture 的**真实形状** + 可调的合成密度。
//!
//! 为什么不是随手造几条字符串：ADR-010 的跨 FFI 形状是「Rust 持有已过滤、
//! 已分桶的结果，渲染侧每 100ms 取一桶」。本 spike 要测的恰恰是这条路径在
//! 60fps 下够不够快，所以必须走真的 [`DanmakuSession`]，而不是在 UI 里
//! 自己攒一个数组。
//!
//! 为什么要合成密度：`rust/crates/bili-api/tests/fixtures/danmaku_seg/` 里
//! 的样本是为**解码正确性**挑的（4 条、2 条），覆盖了 mode/pool/彩色/超长
//! id 等形态，但密度远不足以压渲染。于是：**形状取自 fixture，密度由
//! `--danmaku-rate` 合成**。报告里必须照这个说法写，不能把合成密度说成实测
//! 弹幕量。

use std::path::Path;

use anyhow::{Context, Result};
use bili_danmaku::engine::EngineOptions;
use bili_danmaku::session::DanmakuSession;
use bili_domain::danmaku::{DanmakuElem, DanmakuSegment};
use bili_domain::session::SessionId;

/// 一个分段覆盖 6 分钟（协议如此）。
const SEGMENT_MS: i32 = 6 * 60 * 1000;

pub struct DanmakuSource {
    pub session: DanmakuSession,
    /// 实际入桶条数（过滤后）。
    pub total: usize,
    /// 取了哪些 fixture，报告里要写。
    pub shapes_from: Vec<String>,
    pub synthetic: bool,
}

/// 读 golden fixture 并按 `rate`（条/秒）铺满 `duration_ms`。
///
/// `rate` 为 0 表示不合成，只放 fixture 自己的那几条。
pub fn load(
    fixture_dir: &Path,
    names: &[String],
    rate: f32,
    duration_ms: i32,
    merge: bool,
) -> Result<DanmakuSource> {
    let mut shapes = Vec::new();
    let mut shapes_from = Vec::new();
    for name in names {
        let path = fixture_dir.join(format!("{name}.raw.pb"));
        let bytes = std::fs::read(&path)
            .with_context(|| format!("读不到弹幕 fixture：{}", path.display()))?;
        let segment = bili_api::danmaku::parse_segment(&bytes)
            .with_context(|| format!("解不开弹幕 fixture：{}", path.display()))?;
        if segment.elems.is_empty() {
            // `closed.raw.pb` 就是空的（state=1 的样本），不算错误。
            continue;
        }
        shapes.extend(segment.elems);
        shapes_from.push(name.clone());
    }
    anyhow::ensure!(!shapes.is_empty(), "所有 fixture 都没有弹幕，无法合成");

    let options = EngineOptions {
        merge,
        ..EngineOptions::default()
    };
    let mut session = DanmakuSession::new(SessionId(1), options);

    let synthetic = rate > 0.0;
    if synthetic {
        let step_ms = (1000.0 / rate).max(1.0) as i32;
        let mut by_segment: Vec<Vec<DanmakuElem>> = Vec::new();
        let mut progress = 0;
        let mut shape_idx = 0usize;
        while progress < duration_ms {
            let seg_index = (progress / SEGMENT_MS) as usize;
            while by_segment.len() <= seg_index {
                by_segment.push(Vec::new());
            }
            let mut elem = shapes[shape_idx % shapes.len()].clone();
            shape_idx += 1;
            elem.progress = progress;
            // id 必须各不相同：引擎按 content 合并，但调用方按 id 认弹幕，
            // 复用同一个 id 会让「同一条弹幕出现多次」这种真实 bug 被掩盖。
            elem.id = elem.id.wrapping_add(i64::from(progress));
            elem.id_str = elem.id.to_string();
            by_segment[seg_index].push(elem);
            progress += step_ms;
        }
        for (index, elems) in by_segment.into_iter().enumerate() {
            session.ingest_segment(index as i64, DanmakuSegment { elems, state: 0 });
        }
    } else {
        session.ingest_segment(
            0,
            DanmakuSegment {
                elems: shapes,
                state: 0,
            },
        );
    }

    let total = session.elem_count();
    Ok(DanmakuSource {
        session,
        total,
        shapes_from,
        synthetic,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../rust/crates/bili-api/tests/fixtures/danmaku_seg")
    }

    #[test]
    fn synthesizes_the_requested_rate_from_fixture_shapes() {
        let names = vec!["typical".to_owned(), "rich".to_owned()];
        let source = load(&fixture_dir(), &names, 10.0, 10_000, false).expect("fixture 应当可读");
        // 10 条/秒 × 10 秒。
        assert_eq!(source.total, 100);
        assert!(source.synthetic);
        assert_eq!(source.shapes_from, names);
        // 桶里拿到的必须是真实解码出来的形状，不是构造的字符串。
        let bucket = source.session.bucket(0).expect("0ms 处应当有弹幕");
        assert!(!bucket[0].elem.content.is_empty());
    }

    #[test]
    fn rate_zero_keeps_only_the_fixture_elements() {
        let names = vec!["typical".to_owned()];
        let source = load(&fixture_dir(), &names, 0.0, 10_000, false).expect("fixture 应当可读");
        assert!(!source.synthetic);
        assert_eq!(source.total, 4);
    }

    #[test]
    fn an_empty_fixture_is_not_an_error_on_its_own() {
        let names = vec!["closed".to_owned(), "typical".to_owned()];
        let source = load(&fixture_dir(), &names, 0.0, 10_000, false).expect("fixture 应当可读");
        assert_eq!(source.shapes_from, vec!["typical".to_owned()]);
    }

    #[test]
    fn synthetic_elements_keep_distinct_ids() {
        let names = vec!["typical".to_owned()];
        let source = load(&fixture_dir(), &names, 20.0, 2_000, false).expect("fixture 应当可读");
        let mut ids: Vec<i64> = Vec::new();
        for ms in (0..2_000).step_by(100) {
            if let Some(bucket) = source.session.bucket(ms) {
                ids.extend(bucket.iter().map(|e| e.elem.id));
            }
        }
        let unique: std::collections::HashSet<i64> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "合成弹幕的 id 撞车了");
    }
}
