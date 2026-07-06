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
| `stat` | 0.394s | **0.386s** | 0.671s |
| `open` / `close` | 1.949s | **2.028s** | 2.631s |
| `read` | 0.134s | **0.137s** | 0.401s |
| `write` | 0.187s | **0.190s** | 0.452s |
| `socket` / `close` | 1.156s | **0.461s** | 1.009s |
| `conn` / `send` / `recv` | 0.192s | **0.202s** | 1.007s |
| `mmap` / `munmap` | 0.176s | **0.181s** | 0.725s |

### *Side note on Socket
You may notice that native `socket` creation took `1.156s`, but running it through `mtrace` actually dropped the execution time down to `0.461s`. This is not an error!

On macOS, `libnetwork.dylib` and other userspace XPC daemons (like the macOS Application Firewall or Little Snitch) hook into raw network calls for telemetry and security validation. By using `DYLD_INSERT_LIBRARIES` to aggressively interpose on the lowest-level `libc::socket` stub, `mtrace` forces execution to skip these higher-level Apple telemetry frameworks. The result is a tracer so efficient that it actively accelerates macOS networking by shedding the OS's native userspace telemetry bloat.

I think, at least. I haven't done any testing on that front, but this is my hypothesis. Another theory is that something inside is failing gracefully and just continuing with the call, skipping all the XPC telemetry.