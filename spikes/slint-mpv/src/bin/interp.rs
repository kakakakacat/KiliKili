//! #218 插帧 spike 的桌面入口。用法见 `src/interp/app.rs` 与 README。

#[path = "../interp/mod.rs"]
mod interp;
#[path = "../mpv/mod.rs"]
mod mpv;

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let mut args = interp::app::Args::parse();
    args.raw = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    println!("{}", interp::app::run(args)?);
    Ok(())
}
