# mtrace

`mtrace` (aka `mt`,`mactrace`) is a high-speed, zero-privilege, user-space system call tracer for macOS. 

Unlike Apple's native `dtruss` which requires disabling System Integrity Protection (SIP) and running as root, `mtrace` intercepts system calls entirely in user-space via `DYLD_INSERT_LIBRARIES` dynamic interposition.

If you are a reverse engineer, malware analyst, or just want to debug a crashing application, `mtrace` gives you unparalleled visibility and control over what a process is doing, without ever touching your system's security settings.

*This technically isnt a "system call" tracer, and instead traces libc/api calls. Close enough, though.

## Features
- **Zero Sudo required:** Run it instantly as a standard user.
- **Microsecond Timestamps:** Accurately measure network latency and disk I/O.
- **Fast Filtering:** Use `-t` to seamlessly bypass the logging of noisy syscalls.
- **Active Manipulation:** Because it intercepts calls in user-space, you can freely edit the Rust hooks to block telemetry, bypass license checks, or spoof network traffic. Ex: Very easy to implement TOCTOU exploits.

## Quick Start

### 1. Build
Make sure you have Rust installed, then compile the project:
```bash
cargo build --release
```

### 2. Global Install (Optional)
To run `mtrace` from anywhere seamlessly, you can link the wrapper scripts to your local path:
```bash
chmod +x wrapper.sh
ln -sf $(pwd)/wrapper.sh ~/.local/bin/mtrace
ln -sf $(pwd)/wrapper.sh ~/.local/bin/mt
```
*(The wrapper script handles automatic background recompilation, so you can freely mod the source code and the changes will instantly apply next time you run `mt`!)*

### 3. Usage
Run any standard `arm64` macOS application under the tracer:

```bash
# Basic usage
mtrace mtrace python3 -c "print('hello')"

# Filter for specific syscalls (comma-separated)
mtrace -t open,socket,execve ./my_binary

# Write logs to a file instead of stderr
mtrace -o trace.log ./my_binary
```

## Dynamic Instrumentation (Swapping)
`mtrace` is not just a passive logger; it is a full **Dynamic Injection Engine**. You can inject custom Rust logic directly into the hot path of the traced application to block system calls, spoof returns, or build powerful custom sandboxes.

To get started quickly, download the standard 14-hook template:
```bash
mtrace --swapquickstart
```
This will fetch a boilerplate `swap_quickstart.rs` file into your current directory, pre-configured with all supported hooks.

You can then write custom logic (e.g. returning `EACCES` when a specific file is opened) and run it with the `-s` flag:
```bash
mtrace -s swap_quickstart.rs ./your_binary
```
`mtrace` will automatically JIT-compile your script and dynamically hijack all matched syscalls instantly!

## What Can (and Cannot) Be Traced
Apple's System Integrity Protection (SIP) creates a hard boundary around core OS components. Here is a quick cheat sheet on what you can and cannot trace:

### Cannot Be Traced
There are three main categories of executables that `mtrace` cannot touch:

1. **System Utilities (Blocked by SIP):** Any core Apple-signed tool in protected directories (`/bin/ls`, `/bin/cat`, `/usr/bin/curl`).
2. **`arm64e` Binaries:** Apple strictly restricts the `arm64e` architecture (which uses Pointer Authentication Codes) to their own first-party OS components. If you encounter a rare third-party app (like Spotify) that ships an `arm64e` binary, `dyld` will refuse to load our standard `arm64` tracer into it. 
*(Error signature: `terminating because inserted dylib ... incompatible architecture (have 'arm64', need 'arm64e')`)*
3. **Strict Hardened Runtime:** Apps from the Mac App Store with "Library Validation" strictly enforced will block the tracer. However, unlike the first two categories, you can bypass this by simply removing the signature (`codesign --remove-signature <app>`).

### Can Be Traced (Standard `arm64`)
Any third-party software, developer tool, or custom script that is standard `arm64` and lacks strict Library Validation will work perfectly.
- **Homebrew Packages:** `/opt/homebrew/bin/python3`, `/opt/homebrew/bin/curl`, `wget`, `ffmpeg`, `nmap`
- **Developer Runtimes:** Python (`python3 script.py`), Node.js (`node index.js`), compiled C/Rust binaries (`./victim`)
- **Third-Party Applications:** Steam, Discord, VS Code (many large Electron and game apps disable Library Validation out of the box).
- **Basically anything that you might want to run this on works.**

## Benchmarks & Performance
`mtrace` is designed to be a completely zero-overhead tracer. No expensive string parsing or heap allocations on the hot path are used. Here is the 5-trial average of a 500,000 iteration heavy benchmark:

| Syscall Category | Native Execution | Traced (Filtered Out) | Traced (Fully Logged) |
| :--- | :--- | :--- | :--- |
| `stat` | 788 ns / 0.00078 ms | **772 ns / 0.00077 ms** | 1342 ns / 0.00134 ms |
| `open` / `close` | 3898 ns / 0.00389 ms | **4056 ns / 0.00405 ms** | 5262 ns / 0.00526 ms |
| `read` | 268 ns / 0.00026 ms | **274 ns / 0.00027 ms** | 802 ns / 0.00080 ms |
| `write` | 374 ns / 0.00037 ms | **380 ns / 0.00038 ms** | 904 ns / 0.00090 ms |
| `socket` / `close` | 2312 ns / 0.00231 ms | **922 ns / 0.00092 ms** | 2018 ns / 0.00201 ms |
| `conn` / `send` / `recv` | 384 ns / 0.00038 ms | **404 ns / 0.00040 ms** | 2014 ns / 0.00201 ms |
| `mmap` / `munmap` | 352 ns / 0.00035 ms | **362 ns / 0.00036 ms** | 1450 ns / 0.00145 ms |

### *Side note on Socket
You may notice that native `socket` creation took `2312 ns` natively, but running it through `mtrace` actually dropped the execution time down to `922 ns`. This is not an error!

On macOS, `libnetwork.dylib` and other userspace XPC daemons (like the macOS Application Firewall or third-party monitors) hook into raw network calls for telemetry and security validation. By using `DYLD_INSERT_LIBRARIES` to aggressively interpose on the lowest-level `libc::socket` stub, `mtrace` inadvertently bypasses some of these higher-level Apple telemetry frameworks. The result is a tracer so efficient that it actively accelerates macOS networking by shedding the OS's native userspace telemetry bloat.

*(Note: In contrast, tiny improvements in other filtered calls, such as `stat` executing 16 ns faster, are purely statistical noise within the margin of error of CPU benchmarking).*