// iOS App 的入口：把控制权交给 Rust。winit 在里面调 UIApplicationMain，
// 不会返回。
extern void slint_ios_main(void);

int main(void) {
    slint_ios_main();
    return 0;
}
