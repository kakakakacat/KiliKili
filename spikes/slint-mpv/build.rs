fn main() {
    slint_build::compile("ui/spike.slint").expect("编译 .slint 失败");

    // 实验记录要写明依赖版本（SPIKE-PLAN 的模板里「dependency lockfile」
    // 那一栏）。从 Cargo.lock 里读实际解析到的版本，而不是 Cargo.toml 里
    // 的版本要求——`"1.19"` 与实际用的 1.19.3 不是一回事。
    let version = std::fs::read_to_string("Cargo.lock")
        .ok()
        .and_then(|lock| slint_version(&lock))
        .unwrap_or_else(|| "<读不到 Cargo.lock>".to_owned());
    println!("cargo::rustc-env=SLINT_VERSION_FOR_RECORD={version}");
    println!("cargo::rerun-if-changed=Cargo.lock");
    println!("cargo::rerun-if-changed=ui/spike.slint");
}

fn slint_version(lock: &str) -> Option<String> {
    let mut in_slint = false;
    for line in lock.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            in_slint = false;
        } else if line == "name = \"slint\"" {
            in_slint = true;
        } else if in_slint && let Some(rest) = line.strip_prefix("version = ") {
            return Some(rest.trim_matches('"').to_owned());
        }
    }
    None
}
