fn main() {
    slint_build::compile("ui/app.slint").expect("编译 .slint 失败");
    println!("cargo::rerun-if-changed=ui/app.slint");
}
