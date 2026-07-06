use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};
use std::io::Write;

use core::sync::atomic::{AtomicI32, AtomicU32, AtomicBool, AtomicPtr, Ordering};
use std::ptr;

static LOG_FD: AtomicI32 = AtomicI32::new(2);
static FILTER_MASK: AtomicU32 = AtomicU32::new(0xFFFFFFFF);
static JSON_OUTPUT: AtomicBool = AtomicBool::new(false);
static ECS_OUTPUT: AtomicBool = AtomicBool::new(false);

static USER_ON_OPEN: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static USER_ON_CLOSE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static USER_ON_READ: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static USER_ON_WRITE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static USER_ON_SOCKET: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static USER_ON_CONNECT: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static USER_ON_SEND: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static USER_ON_RECV: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static USER_ON_STAT: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static USER_ON_EXECVE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static USER_ON_FORK: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static USER_ON_EXIT: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static USER_ON_MMAP: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static USER_ON_MUNMAP: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());


#[used]
#[unsafe(link_section = "__DATA,__mod_init_func")]
static INITIALIZE: unsafe extern "C" fn() = {
    unsafe extern "C" fn init() {

        let env_swap = b"MTRACE_SWAP_DYLIB\0".as_ptr() as *const c_char;
        let swap_ptr = unsafe { libc::getenv(env_swap) };
        if !swap_ptr.is_null() {
            let handle = unsafe { libc::dlopen(swap_ptr, libc::RTLD_LAZY | libc::RTLD_LOCAL) };
            if !handle.is_null() {
                macro_rules! load_sym {
                    ($name:expr, $static_var:expr) => {
                        let sym = unsafe { libc::dlsym(handle, concat!($name, "\0").as_ptr() as *const c_char) };
                        if !sym.is_null() { $static_var.store(sym as *mut c_void, Ordering::Relaxed); }
                    };
                }
                load_sym!("on_open", USER_ON_OPEN);
                load_sym!("on_close", USER_ON_CLOSE);
                load_sym!("on_read", USER_ON_READ);
                load_sym!("on_write", USER_ON_WRITE);
                load_sym!("on_socket", USER_ON_SOCKET);
                load_sym!("on_connect", USER_ON_CONNECT);
                load_sym!("on_send", USER_ON_SEND);
                load_sym!("on_recv", USER_ON_RECV);
                load_sym!("on_stat", USER_ON_STAT);
                load_sym!("on_execve", USER_ON_EXECVE);
                load_sym!("on_fork", USER_ON_FORK);
                load_sym!("on_exit", USER_ON_EXIT);
                load_sym!("on_mmap", USER_ON_MMAP);
                load_sym!("on_munmap", USER_ON_MUNMAP);
            }
        }

        let env_out = b"MTRACE_OUTPUT\0".as_ptr() as *const c_char;
        let out_ptr = unsafe { libc::getenv(env_out) };
        if !out_ptr.is_null() {
            let fd = unsafe { libc::open(out_ptr, libc::O_CREAT | libc::O_WRONLY | libc::O_APPEND | libc::O_CLOEXEC, 0o666) };
            if fd >= 0 {
                LOG_FD.store(fd, Ordering::Relaxed);
            }
        }
        
        let env_filter = b"MTRACE_FILTER\0".as_ptr() as *const c_char;
        let filter_ptr = unsafe { libc::getenv(env_filter) };
        if !filter_ptr.is_null() {
            FILTER_MASK.store(0, Ordering::Relaxed);
            if let Ok(filter_str) = core::str::from_utf8(unsafe { CStr::from_ptr(filter_ptr).to_bytes() }) {
                for s in filter_str.split(',') {
                    match s.trim() {
                        "open" => { FILTER_MASK.fetch_or(1 << 0, Ordering::Relaxed); },
                        "close" => { FILTER_MASK.fetch_or(1 << 1, Ordering::Relaxed); },
                        "read" => { FILTER_MASK.fetch_or(1 << 2, Ordering::Relaxed); },
                        "write" => { FILTER_MASK.fetch_or(1 << 3, Ordering::Relaxed); },
                        "socket" => { FILTER_MASK.fetch_or(1 << 4, Ordering::Relaxed); },
                        "connect" => { FILTER_MASK.fetch_or(1 << 5, Ordering::Relaxed); },
                        "send" => { FILTER_MASK.fetch_or(1 << 6, Ordering::Relaxed); },
                        "recv" => { FILTER_MASK.fetch_or(1 << 7, Ordering::Relaxed); },
                        "stat" => { FILTER_MASK.fetch_or(1 << 8, Ordering::Relaxed); },
                        "execve" => { FILTER_MASK.fetch_or(1 << 9, Ordering::Relaxed); },
                        "fork" => { FILTER_MASK.fetch_or(1 << 10, Ordering::Relaxed); },
                        "exit" => { FILTER_MASK.fetch_or(1 << 11, Ordering::Relaxed); },
                        "mmap" => { FILTER_MASK.fetch_or(1 << 12, Ordering::Relaxed); },
                        "munmap" => { FILTER_MASK.fetch_or(1 << 13, Ordering::Relaxed); },
                        _ => {}
                    }
                }
            }
        }

        let env_json = b"MTRACE_JSON\0".as_ptr() as *const c_char;
        let json_ptr = unsafe { libc::getenv(env_json) };
        if !json_ptr.is_null() {
            JSON_OUTPUT.store(true, Ordering::Relaxed);
        }

        let env_ecs = b"MTRACE_ECS\0".as_ptr() as *const c_char;
        let ecs_ptr = unsafe { libc::getenv(env_ecs) };
        if !ecs_ptr.is_null() {
            ECS_OUTPUT.store(true, Ordering::Relaxed);
        }

        if ECS_OUTPUT.load(Ordering::Relaxed) {
            let msg = b"{\"@timestamp\":\"2000-01-01T00:00:00Z\",\"event\":{\"action\":\"init\"},\"message\":\"mactrace active\"}\n\0";
            unsafe { libc::write(LOG_FD.load(Ordering::Relaxed), msg.as_ptr() as *const c_void, msg.len() - 1); }
        } else if JSON_OUTPUT.load(Ordering::Relaxed) {
            let msg = b"{\"event\":\"mactrace_active\"}\n\0";
            unsafe { libc::write(LOG_FD.load(Ordering::Relaxed), msg.as_ptr() as *const c_void, msg.len() - 1); }
        } else {
            let msg = b"[mactrace] Active! Monitoring system calls...\n\0";
            unsafe { libc::write(LOG_FD.load(Ordering::Relaxed), msg.as_ptr() as *const c_void, msg.len() - 1); }
        }
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

#[inline(always)]
fn should_log(bit: u32) -> bool {
    (FILTER_MASK.load(Ordering::Relaxed) & (1 << bit)) != 0
}

struct JsonEscape<'a>(&'a str);
impl<'a> core::fmt::Display for JsonEscape<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for c in self.0.chars() {
            match c {
                '"' => write!(f, "\\\"")?,
                '\\' => write!(f, "\\\\")?,
                '\n' => write!(f, "\\n")?,
                '\r' => write!(f, "\\r")?,
                '\t' => write!(f, "\\t")?,
                c if c < '\x20' => write!(f, "\\u{:04x}", c as u32)?,
                c => write!(f, "{}", c)?,
            }
        }
        Ok(())
    }
}

fn get_timestamp_str(buf: &mut [u8]) -> usize {
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

fn get_iso8601_str(buf: &mut [u8]) -> usize {
    let mut tv = libc::timeval { tv_sec: 0, tv_usec: 0 };
    unsafe { libc::gettimeofday(&mut tv, std::ptr::null_mut()) };
    let mut tm: libc::tm = unsafe { core::mem::zeroed() };
    unsafe { libc::gmtime_r(&tv.tv_sec, &mut tm) };
    
    let total_len = buf.len();
    let mut slice = &mut buf[..];
    let _ = write!(slice, "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday,
        tm.tm_hour, tm.tm_min, tm.tm_sec, tv.tv_usec / 1000
    );
    total_len - slice.len()
}

fn log_event(syscall: &str, args_content: core::fmt::Arguments, plain_msg: core::fmt::Arguments) {
    let mut buf = [0u8; 1024];
    let mut slice = &mut buf[..];
    
    if ECS_OUTPUT.load(Ordering::Relaxed) {
        let mut time_buf = [0u8; 32];
        let time_len = get_iso8601_str(&mut time_buf);
        let time_str = core::str::from_utf8(&time_buf[..time_len]).unwrap_or("");
        let _ = write!(slice, "{{\"@timestamp\":\"{}\",\"event\":{{\"category\":[\"process\"],\"action\":\"{}\"}},\"message\":\"[mactrace] Caught {}\",\"mactrace\":{{{}}}}}\n", time_str, syscall, plain_msg, args_content);
    } else if JSON_OUTPUT.load(Ordering::Relaxed) {
        let mut time_buf = [0u8; 32];
        let time_len = get_timestamp_str(&mut time_buf);
        let time_str = core::str::from_utf8(&time_buf[..time_len]).unwrap_or("");
        let _ = write!(slice, "{{\"timestamp\":\"{}\",\"syscall\":\"{}\",\"args\":{{{}}}}}\n", time_str, syscall, args_content);
    } else {
        let mut time_buf = [0u8; 32];
        let time_len = get_timestamp_str(&mut time_buf);
        let time_str = core::str::from_utf8(&time_buf[..time_len]).unwrap_or("");
        let _ = write!(slice, "[{}] [mactrace] Caught {}\n", time_str, plain_msg);
    }
    
    let len = 1024 - slice.len();
    unsafe { libc::write(LOG_FD.load(Ordering::Relaxed), buf.as_ptr() as *const c_void, len); }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_open(path: *const c_char, oflag: c_int, mode: c_int) -> c_int { unsafe {
    let p = USER_ON_OPEN.load(Ordering::Relaxed);
    if !p.is_null() {
        let func: unsafe extern "C" fn(*const c_char, c_int, c_int) -> c_int = core::mem::transmute(p);
        return func(path, oflag, mode);
    }
    if !should_log(0) { return unsafe { libc::open(path, oflag, mode) } }
    let len = unsafe { libc::strnlen(path, 1024) };
    let path_bytes = unsafe { core::slice::from_raw_parts(path as *const u8, len) };
    let path_str = core::str::from_utf8(path_bytes).unwrap_or("<invalid_utf8>");
    let escaped = JsonEscape(path_str);
    log_event(
        "open",
        format_args!("\"path\":\"{}\",\"oflag\":{},\"mode\":{}", escaped, oflag, mode),
        format_args!("open(\"{}\", {}, {})", escaped, oflag, mode)
    );
    unsafe { libc::open(path, oflag, mode) }
}}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_close(fd: c_int) -> c_int { unsafe {
    let p = USER_ON_CLOSE.load(Ordering::Relaxed);
    if !p.is_null() {
        let func: unsafe extern "C" fn(c_int) -> c_int = core::mem::transmute(p);
        return func(fd);
    }
    if !should_log(1) { return unsafe { libc::close(fd) } }
    log_event(
        "close",
        format_args!("\"fd\":{}", fd),
        format_args!("close({})", fd)
    );
    unsafe { libc::close(fd) }
}}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_read(fd: c_int, buf: *mut c_void, count: usize) -> isize { unsafe {
    let p = USER_ON_READ.load(Ordering::Relaxed);
    if !p.is_null() {
        let func: unsafe extern "C" fn(c_int, *mut c_void, usize) -> isize = core::mem::transmute(p);
        return func(fd, buf, count);
    }
    if !should_log(2) { return unsafe { libc::read(fd, buf, count) } }
    let ret = unsafe { libc::read(fd, buf, count) };
    log_event(
        "read",
        format_args!("\"fd\":{},\"count\":{},\"ret\":{}", fd, count, ret),
        format_args!("read({}, buf, {}) -> {}", fd, count, ret)
    );
    ret
}}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_write(fd: c_int, buf: *const c_void, count: usize) -> isize { unsafe {
    let p = USER_ON_WRITE.load(Ordering::Relaxed);
    if !p.is_null() {
        let func: unsafe extern "C" fn(c_int, *const c_void, usize) -> isize = core::mem::transmute(p);
        return func(fd, buf, count);
    }
    if !should_log(3) { return unsafe { libc::write(fd, buf, count) } }
    log_event(
        "write",
        format_args!("\"fd\":{},\"count\":{}", fd, count),
        format_args!("write({}, buf, {})", fd, count)
    );
    unsafe { libc::write(fd, buf, count) }
}}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_socket(domain: c_int, ty: c_int, protocol: c_int) -> c_int { unsafe {
    let p = USER_ON_SOCKET.load(Ordering::Relaxed);
    if !p.is_null() {
        let func: unsafe extern "C" fn(c_int, c_int, c_int) -> c_int = core::mem::transmute(p);
        return func(domain, ty, protocol);
    }
    if !should_log(4) { return unsafe { libc::socket(domain, ty, protocol) } }
    let ret = unsafe { libc::socket(domain, ty, protocol) };
    log_event(
        "socket",
        format_args!("\"domain\":{},\"type\":{},\"protocol\":{},\"ret\":{}", domain, ty, protocol, ret),
        format_args!("socket({}, {}, {}) -> {}", domain, ty, protocol, ret)
    );
    ret
}}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_connect(socket: c_int, address: *const libc::sockaddr, len: libc::socklen_t) -> c_int { unsafe {
    let p = USER_ON_CONNECT.load(Ordering::Relaxed);
    if !p.is_null() {
        let func: unsafe extern "C" fn(c_int, *const libc::sockaddr, libc::socklen_t) -> c_int = core::mem::transmute(p);
        return func(socket, address, len);
    }
    if !should_log(5) { return unsafe { libc::connect(socket, address, len) } }
    log_event(
        "connect",
        format_args!("\"socket\":{},\"len\":{}", socket, len),
        format_args!("connect({}, address, {})", socket, len)
    );
    unsafe { libc::connect(socket, address, len) }
}}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_send(socket: c_int, buf: *const c_void, len: usize, flags: c_int) -> isize { unsafe {
    let p = USER_ON_SEND.load(Ordering::Relaxed);
    if !p.is_null() {
        let func: unsafe extern "C" fn(c_int, *const c_void, usize, c_int) -> isize = core::mem::transmute(p);
        return func(socket, buf, len, flags);
    }
    if !should_log(6) { return unsafe { libc::send(socket, buf, len, flags) } }
    log_event(
        "send",
        format_args!("\"socket\":{},\"len\":{},\"flags\":{}", socket, len, flags),
        format_args!("send({}, buf, {}, {})", socket, len, flags)
    );
    unsafe { libc::send(socket, buf, len, flags) }
}}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_recv(socket: c_int, buf: *mut c_void, len: usize, flags: c_int) -> isize { unsafe {
    let p = USER_ON_RECV.load(Ordering::Relaxed);
    if !p.is_null() {
        let func: unsafe extern "C" fn(c_int, *mut c_void, usize, c_int) -> isize = core::mem::transmute(p);
        return func(socket, buf, len, flags);
    }
    if !should_log(7) { return unsafe { libc::recv(socket, buf, len, flags) } }
    let ret = unsafe { libc::recv(socket, buf, len, flags) };
    log_event(
        "recv",
        format_args!("\"socket\":{},\"len\":{},\"flags\":{},\"ret\":{}", socket, len, flags, ret),
        format_args!("recv({}, buf, {}, {}) -> {}", socket, len, flags, ret)
    );
    ret
}}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_stat(path: *const c_char, buf: *mut libc::stat) -> c_int { unsafe {
    let p = USER_ON_STAT.load(Ordering::Relaxed);
    if !p.is_null() {
        let func: unsafe extern "C" fn(*const c_char, *mut libc::stat) -> c_int = core::mem::transmute(p);
        return func(path, buf);
    }
    if !should_log(8) { return unsafe { libc::stat(path, buf) } }
    let len = unsafe { libc::strnlen(path, 1024) };
    let path_bytes = unsafe { core::slice::from_raw_parts(path as *const u8, len) };
    let path_str = core::str::from_utf8(path_bytes).unwrap_or("<invalid_utf8>");
    let escaped = JsonEscape(path_str);
    log_event(
        "stat",
        format_args!("\"path\":\"{}\"", escaped),
        format_args!("stat(\"{}\", buf)", escaped)
    );
    unsafe { libc::stat(path, buf) }
}}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_execve(path: *const c_char, argv: *const *mut c_char, envp: *const *mut c_char) -> c_int { unsafe {
    let p = USER_ON_EXECVE.load(Ordering::Relaxed);
    if !p.is_null() {
        let func: unsafe extern "C" fn(*const c_char, *const *mut c_char, *const *mut c_char) -> c_int = core::mem::transmute(p);
        return func(path, argv, envp);
    }
    if !should_log(9) { return unsafe { libc::execve(path, argv, envp) } }
    let len = unsafe { libc::strnlen(path, 1024) };
    let path_bytes = unsafe { core::slice::from_raw_parts(path as *const u8, len) };
    let path_str = core::str::from_utf8(path_bytes).unwrap_or("<invalid_utf8>");
    let escaped = JsonEscape(path_str);
    log_event(
        "execve",
        format_args!("\"path\":\"{}\"", escaped),
        format_args!("execve(\"{}\", argv, envp)", escaped)
    );
    unsafe { libc::execve(path, argv, envp) }
}}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_fork() -> libc::pid_t { unsafe {
    let p = USER_ON_FORK.load(Ordering::Relaxed);
    if !p.is_null() {
        let func: unsafe extern "C" fn() -> libc::pid_t = core::mem::transmute(p);
        return func();
    }
    if !should_log(10) { return unsafe { libc::fork() } }
    log_event(
        "fork",
        format_args!(""),
        format_args!("fork()")
    );
    unsafe { libc::fork() }
}}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_exit(status: c_int) -> ! { unsafe {
    let p = USER_ON_EXIT.load(Ordering::Relaxed);
    if !p.is_null() {
        let func: unsafe extern "C" fn(c_int) -> ! = core::mem::transmute(p);
        return func(status);
    }
    if should_log(11) {
        log_event(
            "exit",
            format_args!("\"status\":{}", status),
            format_args!("exit({})", status)
        );
    }
    unsafe { libc::exit(status) }
}}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_mmap(addr: *mut c_void, len: usize, prot: c_int, flags: c_int, fd: c_int, offset: libc::off_t) -> *mut c_void { unsafe {
    let p = USER_ON_MMAP.load(Ordering::Relaxed);
    if !p.is_null() {
        let func: unsafe extern "C" fn(*mut c_void, usize, c_int, c_int, c_int, libc::off_t) -> *mut c_void = core::mem::transmute(p);
        return func(addr, len, prot, flags, fd, offset);
    }
    if !should_log(12) { return unsafe { libc::mmap(addr, len, prot, flags, fd, offset) } }
    log_event(
        "mmap",
        format_args!("\"len\":{},\"prot\":{},\"flags\":{},\"fd\":{},\"offset\":{}", len, prot, flags, fd, offset),
        format_args!("mmap(addr, {}, {}, {}, {}, {})", len, prot, flags, fd, offset)
    );
    unsafe { libc::mmap(addr, len, prot, flags, fd, offset) }
}}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_munmap(addr: *mut c_void, len: usize) -> c_int { unsafe {
    let p = USER_ON_MUNMAP.load(Ordering::Relaxed);
    if !p.is_null() {
        let func: unsafe extern "C" fn(*mut c_void, usize) -> c_int = core::mem::transmute(p);
        return func(addr, len);
    }
    if !should_log(13) { return unsafe { libc::munmap(addr, len) } }
    log_event(
        "munmap",
        format_args!("\"len\":{}", len),
        format_args!("munmap(addr, {})", len)
    );
    unsafe { libc::munmap(addr, len) }
}}