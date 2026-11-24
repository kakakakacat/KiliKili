//! `ios/bridge.m` 导出的 C 函数。说明见那边。

use std::ffi::{c_char, c_int, c_uint, c_void};

unsafe extern "C" {
    pub fn spike_app_state() -> c_int;
    pub fn spike_cf_release(r: *mut c_void);
    pub fn spike_mtl_texture(
        pixbuf: *mut c_void,
        device: *mut c_void,
        keep: *mut *mut c_void,
    ) -> *mut c_void;

    pub fn spike_gl_init() -> c_int;
    pub fn spike_gl_make_current() -> c_int;
    pub fn spike_gl_proc(ctx: *mut c_void, name: *const c_char) -> *mut c_void;
    pub fn spike_gl_flush(finish: c_int);
    pub fn spike_target_create(w: c_int, h: c_int, fbo: *mut c_uint) -> *mut c_void;
    pub fn spike_target_pixbuf(target: *mut c_void) -> *mut c_void;

    pub fn spike_pip_setup(set_playing: extern "C" fn(c_int)) -> c_int;
    pub fn spike_pip_timeline(position: f64, duration: f64, paused: c_int);
    pub fn spike_pip_active() -> c_int;
    pub fn spike_pip_possible() -> c_int;
    pub fn spike_pip_toggle();
    pub fn spike_pip_layer_status() -> c_int;
    pub fn spike_pip_enqueue(pixbuf: *mut c_void);

    pub fn spike_avp_start(
        video: *const c_char,
        audio: *const c_char,
        ua: *const c_char,
        referer: *const c_char,
    ) -> c_int;
    pub fn spike_avp_status() -> *const c_char;
    pub fn spike_avp_copy_frame() -> *mut c_void;
    pub fn spike_avp_info(pos: *mut f64, dur: *mut f64, item_status: *mut c_int);
    pub fn spike_avp_set_playing(playing: c_int);
    pub fn spike_avp_av_offset() -> f64;

    pub fn spike_displaylink_start(tick: extern "C" fn(f64, f64));
    pub fn spike_displaylink_rate(preferred: f32);
    pub fn spike_screen_max_fps() -> c_int;
    pub fn spike_thermal_state() -> c_int;
}
