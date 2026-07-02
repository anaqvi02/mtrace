# mtrace

`mtrace` is a high-speed, zero-privilege, user-space system call tracer for macOS. 

Unlike Apple's native `dtruss` which requires disabling System Integrity Protection (SIP) and running as root, `mtrace` intercepts system calls entirely in user-space via `DYLD_INSERT_LIBRARIES` dynamic interposition.

If you are a reverse engineer, malware analyst, or just want to debug a crashing application, `mtrace` gives you unparalleled visibility and control over what a process is doing, without ever touching your system's security settings.

## Features
- **Zero Sudo:** Run it instantly as a standard user.
- **Microsecond Timestamps:** Accurately measure network latency and disk I/O.
- **Fast Filtering:** Use `-t` to seamlessly bypass the logging of noisy syscalls.
- **Active Manipulation:** Because it intercepts calls in user-space, you can freely edit the Rust hooks to block telemetry, bypass license checks, or spoof network traffic.

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
mtrace ls

# Filter for specific syscalls (comma-separated)
mtrace -t open,socket,execve ./my_binary

# Write logs to a file instead of stderr
mtrace -o trace.log ./my_binary
```

## How to Test
We have included a dummy `victim` program in the `examples/` directory that generates fake file I/O, memory mappings, and network traffic.

```bash
cd examples
gcc victim.c -o victim
cd ..
mtrace -t open,socket ./examples/victim
```

## The "Gotcha": Hardened Runtime (Library Validation)
Apple strictly restricts `arm64e` system binaries (like `/bin/cat` or Safari). However, almost all third-party software (Steam, Discord, VS Code, Homebrew packages) are standard `arm64` and can be traced.

If you encounter a Mac App Store application that blocks injection because of Apple's **Library Validation** entitlement, you can easily strip its signature to trace it anyway:
```bash
codesign --remove-signature /Applications/StrictApp.app
codesign --force --sign - /Applications/StrictApp.app
```

## Adding New Hooks
`mtrace` makes it incredibly easy to add new hooks. Just open `src/lib.rs` and use the `interpose!` macro to intercept any function found in `libc`:
```rust
interpose!(my_unlink, libc::unlink),
```

## What Can (and Cannot) Be Traced
Apple's System Integrity Protection (SIP) creates a hard boundary around core OS components. Here is a quick cheat sheet on what you can and cannot trace:

### ❌ Cannot Be Traced (Blocked by SIP)
Core Apple-signed system utilities and applications compiled for `arm64e` with restricted entitlements will immediately crash if you try to trace them.
- **System Utilities:** `/bin/ls`, `/bin/cat`, `/bin/bash`
- **System Network Tools:** `/usr/bin/curl`, `/usr/bin/ssh`
- **First-Party Apple Apps:** Safari, Finder, Activity Monitor

*(Error signature: `terminating because inserted dylib ... incompatible architecture (have 'arm64', need 'arm64e')`)*

### ✅ Can Be Traced (Standard `arm64`)
Any third-party software, developer tool, or custom script that is standard `arm64` and lacks strict Library Validation will work perfectly.
- **Homebrew Packages:** `/opt/homebrew/bin/python3`, `/opt/homebrew/bin/curl`, `wget`, `ffmpeg`, `nmap`
- **Developer Runtimes:** Python (`python3 script.py`), Node.js (`node index.js`), compiled C/Rust binaries (`./victim`)
- **Third-Party Applications:** Steam, Discord, VS Code, Spotify (often require stripping their signature first if downloaded from the Mac App Store).
