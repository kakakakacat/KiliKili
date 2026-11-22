//! 桌面入口。真正的实验流程在 `app::run`，Android 那条路走的是同一个函数
//! （见 `src/lib.rs`）。这里只负责解析命令行。

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    slint_ui_spike::app::run(
        slint_ui_spike::app::Args::parse(),
        std::env::args().collect(),
    )
}
