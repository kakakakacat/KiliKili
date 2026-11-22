fn main() {
    slint_build::compile("ui/ui_spike.slint").expect("编译 .slint 失败");

    // 实验记录要写明依赖版本（SPIKE-PLAN 的模板里「dependency lockfile」
    // 那一栏）。从 Cargo.lock 里读实际解析到的版本，而不是 Cargo.toml 里
    // 的版本要求——`"1.19"` 与实际用的 1.19.x 不是一回事。
    let version = std::fs::read_to_string("Cargo.lock")
        .ok()
        .and_then(|lock| locked_version(&lock, "slint"))
        .unwrap_or_else(|| "<读不到 Cargo.lock>".to_owned());
    println!("cargo::rustc-env=SLINT_VERSION_FOR_RECORD={version}");
    println!("cargo::rerun-if-changed=Cargo.lock");
    println!("cargo::rerun-if-changed=ui/ui_spike.slint");
}

fn locked_version(lock: &str, package: &str) -> Option<String> {
    let needle = format!("name = \"{package}\"");
    let mut inside = false;
    for line in lock.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            inside = false;
        } else if line == needle {
            inside = true;
        } else if inside && let Some(rest) = line.strip_prefix("version = ") {
            return Some(rest.trim_matches('"').to_owned());
        }
    }
    None
}
