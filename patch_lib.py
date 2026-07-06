import re
import sys

def main():
    with open("src/lib.rs", "r") as f:
        content = f.read()

    # 1. Add Atomics
    atomic_imports = "use core::sync::atomic::{AtomicI32, AtomicU32, AtomicBool, AtomicPtr, Ordering};\nuse std::ptr;"
    content = content.replace("use core::sync::atomic::{AtomicI32, AtomicU32, AtomicBool, Ordering};", atomic_imports)

    globals_block = """
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
"""
    content = content.replace("static ECS_OUTPUT: AtomicBool = AtomicBool::new(false);", "static ECS_OUTPUT: AtomicBool = AtomicBool::new(false);\n" + globals_block)

    # 2. Add init loading
    init_loading = """
        let env_swap = b"MTRACE_SWAP_DYLIB\\0".as_ptr() as *const c_char;
        let swap_ptr = unsafe { libc::getenv(env_swap) };
        if !swap_ptr.is_null() {
            let handle = unsafe { libc::dlopen(swap_ptr, libc::RTLD_LAZY | libc::RTLD_LOCAL) };
            if !handle.is_null() {
                macro_rules! load_sym {
                    ($name:expr, $static_var:expr) => {
                        let sym = unsafe { libc::dlsym(handle, concat!($name, "\\0").as_ptr() as *const c_char) };
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
"""
    # Insert at beginning of init()
    content = content.replace("    unsafe extern \"C\" fn init() {\n        let env_out = b\"MTRACE_OUTPUT", "    unsafe extern \"C\" fn init() {\n" + init_loading + "\n        let env_out = b\"MTRACE_OUTPUT")

    # 3. Patch the hooks
    hooks = [
        ("my_open", "USER_ON_OPEN", "unsafe extern \"C\" fn(*const c_char, c_int, c_int) -> c_int"),
        ("my_close", "USER_ON_CLOSE", "unsafe extern \"C\" fn(c_int) -> c_int"),
        ("my_read", "USER_ON_READ", "unsafe extern \"C\" fn(c_int, *mut c_void, usize) -> isize"),
        ("my_write", "USER_ON_WRITE", "unsafe extern \"C\" fn(c_int, *const c_void, usize) -> isize"),
        ("my_socket", "USER_ON_SOCKET", "unsafe extern \"C\" fn(c_int, c_int, c_int) -> c_int"),
        ("my_connect", "USER_ON_CONNECT", "unsafe extern \"C\" fn(c_int, *const libc::sockaddr, libc::socklen_t) -> c_int"),
        ("my_send", "USER_ON_SEND", "unsafe extern \"C\" fn(c_int, *const c_void, usize, c_int) -> isize"),
        ("my_recv", "USER_ON_RECV", "unsafe extern \"C\" fn(c_int, *mut c_void, usize, c_int) -> isize"),
        ("my_stat", "USER_ON_STAT", "unsafe extern \"C\" fn(*const c_char, *mut libc::stat) -> c_int"),
        ("my_execve", "USER_ON_EXECVE", "unsafe extern \"C\" fn(*const c_char, *const *mut c_char, *const *mut c_char) -> c_int"),
        ("my_fork", "USER_ON_FORK", "unsafe extern \"C\" fn() -> libc::pid_t"),
        ("my_exit", "USER_ON_EXIT", "unsafe extern \"C\" fn(c_int) -> !"),
        ("my_mmap", "USER_ON_MMAP", "unsafe extern \"C\" fn(*mut c_void, usize, c_int, c_int, c_int, libc::off_t) -> *mut c_void"),
        ("my_munmap", "USER_ON_MUNMAP", "unsafe extern \"C\" fn(*mut c_void, usize) -> c_int"),
    ]

    for hook_name, global_var, signature in hooks:
        pattern = r"(pub unsafe extern \"C\" fn " + hook_name + r"\((.*?)\)(.*?)\{)"
        
        args_str = ""
        # extract param names for forwarding call
        def replacer(match):
            nonlocal args_str
            signature_text = match.group(1)
            params = match.group(2)
            # extract just the param names
            arg_names = []
            for p in params.split(","):
                if p.strip() == "": continue
                name = p.split(":")[0].strip()
                arg_names.append(name)
            args_str = ", ".join(arg_names)
            
            inject = f"""
    let p = {global_var}.load(Ordering::Relaxed);
    if !p.is_null() {{
        let func: {signature} = core::mem::transmute(p);
        return func({args_str});
    }}"""
            return signature_text + inject

        content = re.sub(pattern, replacer, content, count=1)

    with open("src/lib.rs", "w") as f:
        f.write(content)

if __name__ == "__main__":
    main()
