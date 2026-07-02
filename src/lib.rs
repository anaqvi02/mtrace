use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};
use std::io::Write;

#[used]
#[unsafe(link_section = "__DATA,__mod_init_func")]
static INITIALIZE: unsafe extern "C" fn() = {
    unsafe extern "C" fn init() {
        let msg = b"[mactrace] Parasite active. Monitoring system calls...\n\0";
        unsafe { libc::write(2, msg.as_ptr() as *const c_void, msg.len() - 1); }
    }
    init
};

#[repr(C)]
pub struct Interpose {
    replacement: *const (),
    replacee: *const (),
}
unsafe impl Sync for Interpose {}

#[used]
#[unsafe(link_section = "__DATA,__interpose")]
pub static INTERPOSE_ARRAY: [Interpose; 8] = [
    Interpose {
        replacement: my_open as *const (),
        replacee: libc::open as *const (),
    },
    Interpose {
        replacement: my_close as *const (),
        replacee: libc::close as *const (),
    },
    Interpose {
        replacement: my_read as *const (),
        replacee: libc::read as *const (),
    },
    Interpose {
        replacement: my_write as *const (),
        replacee: libc::write as *const (),
    },
    Interpose {
        replacement: my_socket as *const (),
        replacee: libc::socket as *const (),
    },
    Interpose {
        replacement: my_connect as *const (),
        replacee: libc::connect as *const (),
    },
    Interpose {
        replacement: my_send as *const (),
        replacee: libc::send as *const (),
    },
    Interpose {
        replacement: my_recv as *const (),
        replacee: libc::recv as *const (),
    },
];

fn log_msg(msg: core::fmt::Arguments) {
    let mut buf = [0u8; 256];
    let mut slice = &mut buf[..];
    let _ = write!(slice, "{}\n", msg);
    let len = 256 - slice.len();
    unsafe { libc::write(2, buf.as_ptr() as *const c_void, len); }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_open(path: *const c_char, oflag: c_int, mode: c_int) -> c_int {
    let path_str = unsafe { CStr::from_ptr(path).to_string_lossy() };
    log_msg(format_args!("[mactrace] Caught open(\"{}\", {}, {})", path_str, oflag, mode));
    unsafe { libc::open(path, oflag, mode) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_close(fd: c_int) -> c_int {
    log_msg(format_args!("[mactrace] Caught close({})", fd));
    unsafe { libc::close(fd) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_read(fd: c_int, buf: *mut c_void, count: usize) -> isize {
    let ret = unsafe { libc::read(fd, buf, count) };
    log_msg(format_args!("[mactrace] Caught read({}, buf, {}) -> {}", fd, count, ret));
    ret
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_write(fd: c_int, buf: *const c_void, count: usize) -> isize {
    log_msg(format_args!("[mactrace] Caught write({}, buf, {})", fd, count));
    unsafe { libc::write(fd, buf, count) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_socket(domain: c_int, ty: c_int, protocol: c_int) -> c_int {
    let ret = unsafe { libc::socket(domain, ty, protocol) };
    log_msg(format_args!("[mactrace] Caught socket({}, {}, {}) -> {}", domain, ty, protocol, ret));
    ret
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_connect(socket: c_int, address: *const libc::sockaddr, len: libc::socklen_t) -> c_int {
    log_msg(format_args!("[mactrace] Caught connect({}, address, {})", socket, len));
    unsafe { libc::connect(socket, address, len) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_send(socket: c_int, buf: *const c_void, len: usize, flags: c_int) -> isize {
    log_msg(format_args!("[mactrace] Caught send({}, buf, {}, {})", socket, len, flags));
    unsafe { libc::send(socket, buf, len, flags) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_recv(socket: c_int, buf: *mut c_void, len: usize, flags: c_int) -> isize {
    let ret = unsafe { libc::recv(socket, buf, len, flags) };
    log_msg(format_args!("[mactrace] Caught recv({}, buf, {}, {}) -> {}", socket, len, flags, ret));
    ret
}