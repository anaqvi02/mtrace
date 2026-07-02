use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};
use std::io::Write;

static mut LOG_FD: c_int = 2;
static mut FILTER_PTR: *const c_char = std::ptr::null();

#[used]
#[unsafe(link_section = "__DATA,__mod_init_func")]
static INITIALIZE: unsafe extern "C" fn() = {
    unsafe extern "C" fn init() {
        let env_out = b"MTRACE_OUTPUT\0".as_ptr() as *const c_char;
        let out_ptr = unsafe { libc::getenv(env_out) };
        if !out_ptr.is_null() {
            let fd = unsafe { libc::open(out_ptr, libc::O_CREAT | libc::O_WRONLY | libc::O_APPEND | libc::O_CLOEXEC, 0o666) };
            if fd >= 0 {
                unsafe { LOG_FD = fd; }
            }
        }
        
        let env_filter = b"MTRACE_FILTER\0".as_ptr() as *const c_char;
        let filter_ptr = unsafe { libc::getenv(env_filter) };
        if !filter_ptr.is_null() {
            unsafe { FILTER_PTR = filter_ptr; }
        }

        let msg = b"[mactrace] Active! Monitoring system calls...\n\0";
        unsafe { libc::write(LOG_FD, msg.as_ptr() as *const c_void, msg.len() - 1); }
    }
    init
};

#[repr(C)]
pub struct Interpose {
    replacement: *const (),
    replacee: *const (),
}
unsafe impl Sync for Interpose {}

macro_rules! interpose {
    ($rep:ident, $orig:path) => {
        Interpose {
            replacement: $rep as *const (),
            replacee: $orig as *const (),
        }
    };
}

#[used]
#[unsafe(link_section = "__DATA,__interpose")]
pub static INTERPOSE_ARRAY: [Interpose; 14] = [
    interpose!(my_open, libc::open),
    interpose!(my_close, libc::close),
    interpose!(my_read, libc::read),
    interpose!(my_write, libc::write),
    interpose!(my_socket, libc::socket),
    interpose!(my_connect, libc::connect),
    interpose!(my_send, libc::send),
    interpose!(my_recv, libc::recv),
    interpose!(my_stat, libc::stat),
    interpose!(my_execve, libc::execve),
    interpose!(my_fork, libc::fork),
    interpose!(my_exit, libc::exit),
    interpose!(my_mmap, libc::mmap),
    interpose!(my_munmap, libc::munmap),
];

fn should_log(name: &str) -> bool {
    unsafe {
        if FILTER_PTR.is_null() {
            return true;
        }
        if let Ok(filter_str) = core::str::from_utf8(CStr::from_ptr(FILTER_PTR).to_bytes()) {
            filter_str.split(',').any(|s| s.trim() == name)
        } else {
            true
        }
    }
}

fn get_timestamp_str(buf: &mut [u8]) -> usize { //this is jus for logging
    let mut tv = libc::timeval { tv_sec: 0, tv_usec: 0 };
    unsafe { libc::gettimeofday(&mut tv, std::ptr::null_mut()) };
    
    let sec = tv.tv_sec % 86400;
    let h = (sec / 3600) % 24;
    let m = (sec / 60) % 60;
    let s = sec % 60;
    
    let total_len = buf.len();
    let mut slice = &mut buf[..];
    let _ = write!(slice, "{:02}:{:02}:{:02}.{:06}", h, m, s, tv.tv_usec);
    total_len - slice.len()
}

fn log_msg(msg: core::fmt::Arguments) { //this is also just logging
    let mut time_buf = [0u8; 32];
    let time_len = get_timestamp_str(&mut time_buf);
    let time_str = core::str::from_utf8(&time_buf[..time_len]).unwrap_or("");

    let mut buf = [0u8; 512];
    let mut slice = &mut buf[..];
    let _ = write!(slice, "[{}] {}\n", time_str, msg);
    let len = 512 - slice.len();
    unsafe { libc::write(LOG_FD, buf.as_ptr() as *const c_void, len); }
}

#[unsafe(no_mangle)] //everything past this is interception
pub unsafe extern "C" fn my_open(path: *const c_char, oflag: c_int, mode: c_int) -> c_int {
    if !should_log("open") { return unsafe { libc::open(path, oflag, mode) } }
    let path_str = unsafe { CStr::from_ptr(path).to_string_lossy() };
    log_msg(format_args!("[mactrace] Caught open(\"{}\", {}, {})", path_str, oflag, mode));
    unsafe { libc::open(path, oflag, mode) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_close(fd: c_int) -> c_int {
    if !should_log("close") { return unsafe { libc::close(fd) } }
    log_msg(format_args!("[mactrace] Caught close({})", fd));
    unsafe { libc::close(fd) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_read(fd: c_int, buf: *mut c_void, count: usize) -> isize {
    if !should_log("read") { return unsafe { libc::read(fd, buf, count) } }
    let ret = unsafe { libc::read(fd, buf, count) };
    log_msg(format_args!("[mactrace] Caught read({}, buf, {}) -> {}", fd, count, ret));
    ret
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_write(fd: c_int, buf: *const c_void, count: usize) -> isize {
    if !should_log("write") { return unsafe { libc::write(fd, buf, count) } }
    log_msg(format_args!("[mactrace] Caught write({}, buf, {})", fd, count));
    unsafe { libc::write(fd, buf, count) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_socket(domain: c_int, ty: c_int, protocol: c_int) -> c_int {
    if !should_log("socket") { return unsafe { libc::socket(domain, ty, protocol) } }
    let ret = unsafe { libc::socket(domain, ty, protocol) };
    log_msg(format_args!("[mactrace] Caught socket({}, {}, {}) -> {}", domain, ty, protocol, ret));
    ret
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_connect(socket: c_int, address: *const libc::sockaddr, len: libc::socklen_t) -> c_int {
    if !should_log("connect") { return unsafe { libc::connect(socket, address, len) } }
    log_msg(format_args!("[mactrace] Caught connect({}, address, {})", socket, len));
    unsafe { libc::connect(socket, address, len) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_send(socket: c_int, buf: *const c_void, len: usize, flags: c_int) -> isize {
    if !should_log("send") { return unsafe { libc::send(socket, buf, len, flags) } }
    log_msg(format_args!("[mactrace] Caught send({}, buf, {}, {})", socket, len, flags));
    unsafe { libc::send(socket, buf, len, flags) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_recv(socket: c_int, buf: *mut c_void, len: usize, flags: c_int) -> isize {
    if !should_log("recv") { return unsafe { libc::recv(socket, buf, len, flags) } }
    let ret = unsafe { libc::recv(socket, buf, len, flags) };
    log_msg(format_args!("[mactrace] Caught recv({}, buf, {}, {}) -> {}", socket, len, flags, ret));
    ret
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_stat(path: *const c_char, buf: *mut libc::stat) -> c_int {
    if !should_log("stat") { return unsafe { libc::stat(path, buf) } }
    let path_str = unsafe { CStr::from_ptr(path).to_string_lossy() };
    log_msg(format_args!("[mactrace] Caught stat(\"{}\", buf)", path_str));
    unsafe { libc::stat(path, buf) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_execve(path: *const c_char, argv: *const *mut c_char, envp: *const *mut c_char) -> c_int {
    if !should_log("execve") { return unsafe { libc::execve(path, argv, envp) } }
    let path_str = unsafe { CStr::from_ptr(path).to_string_lossy() };
    log_msg(format_args!("[mactrace] Caught execve(\"{}\", argv, envp)", path_str));
    unsafe { libc::execve(path, argv, envp) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_fork() -> libc::pid_t {
    if !should_log("fork") { return unsafe { libc::fork() } }
    log_msg(format_args!("[mactrace] Caught fork()"));
    unsafe { libc::fork() }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_exit(status: c_int) -> ! {
    if should_log("exit") {
        log_msg(format_args!("[mactrace] Caught exit({})", status));
    }
    unsafe { libc::exit(status) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_mmap(addr: *mut c_void, len: usize, prot: c_int, flags: c_int, fd: c_int, offset: libc::off_t) -> *mut c_void {
    if !should_log("mmap") { return unsafe { libc::mmap(addr, len, prot, flags, fd, offset) } }
    log_msg(format_args!("[mactrace] Caught mmap(addr, {}, {}, {}, {}, {})", len, prot, flags, fd, offset));
    unsafe { libc::mmap(addr, len, prot, flags, fd, offset) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_munmap(addr: *mut c_void, len: usize) -> c_int {
    if !should_log("munmap") { return unsafe { libc::munmap(addr, len) } }
    log_msg(format_args!("[mactrace] Caught munmap(addr, {})", len));
    unsafe { libc::munmap(addr, len) }
}