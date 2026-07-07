use std::process::Command;
use std::env;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::atomic::{AtomicU32, Ordering};
use std::path::Path;
use std::fs;

static CHILD_PID: AtomicU32 = AtomicU32::new(0);

extern "C" fn handle_signal(_sig: libc::c_int) {
    let pid = CHILD_PID.load(Ordering::Relaxed);
    if pid > 0 {
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGKILL);
        }
    }
    unsafe { libc::_exit(1) }
}

fn print_help() {
    println!("mtrace - High-speed macOS user-space system call tracer");
    println!("");
    println!("Usage: mtrace [OPTIONS] <command> [args...]");
    println!("");
    println!("Options:");
    println!("  -o, --output <file>    Write output to a specific file instead of stderr");
    println!("  -t, --trace <calls>    Comma-separated list of syscalls to intercept (e.g. open,read)");
    println!("  -j, --json             Export logs in NDJSON format");
    println!("  -e, --ecs              Export logs in Elastic Common Schema (ECS) JSON format");
    println!("  -s, --swap <file.rs>   JIT compile and inject a custom Rust interceptor logic file");
    println!("  --strip                Automatically copy and ad-hoc resign the target to bypass macOS SIP restrictions");
    println!("                         (WARNING: Running a stripped clone may corrupt the original app's shared data or caches. Only use this if you know what you're doing)");
    println!("  --swapquickstart       Download a template swap.rs file to the current directory");
    println!("  -h, --help             Print this help message and exit");
    println!("");
    println!("Example:");
    println!("  mtrace -t open,socket -j -o trace.json curl http://example.com");
}

fn is_sip_enabled() -> bool {
    let output = Command::new("csrutil").arg("status").output().unwrap_or_else(|_| {
        Command::new("true").output().unwrap()
    });
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.contains("enabled")
}

fn has_hardened_runtime(binary_path: &str) -> bool {
    let output = Command::new("codesign")
        .args(["-dvv", binary_path])
        .output()
        .unwrap_or_else(|_| Command::new("true").output().unwrap());
    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr.contains("flags=0x10000(runtime)")
}

fn has_dyld_entitlement(binary_path: &str) -> bool {
    let output = Command::new("codesign")
        .args(["-d", "--entitlements", ":-", binary_path])
        .output()
        .unwrap_or_else(|_| Command::new("true").output().unwrap());
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.contains("com.apple.security.cs.allow-dyld-environment-variables")
}

fn check_sip_and_codesign(binary_path: &str) {
    if !is_sip_enabled() { return; }
    if has_hardened_runtime(binary_path) && !has_dyld_entitlement(binary_path) {
        eprintln!("");
        eprintln!("⚠️  [mt] WARNING: SIP is enabled and this binary enforces the Hardened Runtime.");
        eprintln!("⚠️  [mt] macOS will silently block DYLD_INSERT_LIBRARIES.");
        eprintln!("💡 [mt] TIP: Run with the '--strip' flag to automatically create and trace an unsigned copy!");
        eprintln!("");
    }
}

fn strip_and_copy(binary_path: &str) -> (String, String) {
    let path = Path::new(binary_path);
    if !path.exists() {
        eprintln!("[mt] Error: Binary not found at {}", binary_path);
        std::process::exit(1);
    }

    let mut app_root = None;
    let mut current = path;
    while let Some(parent) = current.parent() {
        if let Some(ext) = current.extension() {
            if ext == "app" {
                app_root = Some(current);
                break;
            }
        }
        current = parent;
    }

    let target_dir = Path::new("/tmp/mtrace_targets");
    if !target_dir.exists() {
        let _ = fs::create_dir_all(target_dir);
    }

    if let Some(app) = app_root {
        let app_name = app.file_name().unwrap().to_string_lossy();
        let dest_app = target_dir.join(app_name.as_ref());
        
        eprintln!("[mt] Copying {} to {}...", app.display(), dest_app.display());
        let _ = Command::new("rm").args(["-rf", dest_app.to_str().unwrap()]).status();
        let status = Command::new("cp").args(["-R", app.to_str().unwrap(), dest_app.to_str().unwrap()]).status();
        
        if status.is_err() || !status.unwrap().success() {
            eprintln!("[mt] Error: Failed to copy app bundle");
            std::process::exit(1);
        }

        eprintln!("[mt] Recursively removing signatures and ad-hoc resigning...");
        let script = format!(
            "find '{0}' -type f -exec codesign --remove-signature {{}} \\; 2>/dev/null; \
             find '{0}' -type f -exec codesign --force -s - {{}} \\; 2>/dev/null; \
             codesign --force -s - '{0}'",
            dest_app.to_str().unwrap()
        );
        let status = Command::new("bash").args(["-c", &script]).status();
        
        if status.is_err() || !status.unwrap().success() {
            eprintln!("[mt] Warning: Some signatures may not have been cleanly removed.");
        }

        let rel_path = path.strip_prefix(app).unwrap();
        return (dest_app.join(rel_path).to_string_lossy().into_owned(), dest_app.to_string_lossy().into_owned());

    } else {
        let binary_name = path.file_name().unwrap().to_string_lossy();
        let dest_bin = target_dir.join(binary_name.as_ref());
        
        eprintln!("[mt] Copying {} to {}...", path.display(), dest_bin.display());
        let _ = fs::copy(path, &dest_bin);
        
        eprintln!("[mt] Removing signature and ad-hoc resigning...");
        let _ = Command::new("codesign")
            .args(["--force", "-s", "-", dest_bin.to_str().unwrap()])
            .status();

        let _ = Command::new("chmod").args(["+x", dest_bin.to_str().unwrap()]).status();

        return (dest_bin.to_string_lossy().into_owned(), dest_bin.to_string_lossy().into_owned());
    }
}

fn main() -> io::Result<()> {
    let mut dylib_path = env::current_exe()?;
    dylib_path.set_file_name("libmactrace_lib.dylib");

    if !dylib_path.exists() {
        eprintln!("[mt] Error: Dylib not found at {}", dylib_path.display());
        eprintln!("[mt] Make sure you built the project with `cargo build`");
        std::process::exit(1);
    }

    let mut args: Vec<String> = env::args().skip(1).collect();
    let mut output_file = None;
    let mut trace_filter = None;
    let mut swap_file = None;
    let mut json_output = false;
    let mut ecs_output = false;
    let mut strip_signature = false;
    let mut ndump = false;

    if args.is_empty() {
        print_help();
        std::process::exit(1);
    }

    while !args.is_empty() {
        if args[0] == "-o" || args[0] == "--output" {
            args.remove(0);
            if args.is_empty() {
                eprintln!("Error: -o requires a file path");
                std::process::exit(1);
            }
            output_file = Some(args.remove(0));
        } else if args[0] == "-t" || args[0] == "--trace" {
            args.remove(0);
            if args.is_empty() {
                eprintln!("Error: -t requires a comma-separated list of syscalls");
                std::process::exit(1);
            }
            trace_filter = Some(args.remove(0));
        } else if args[0] == "-j" || args[0] == "--json" {
            args.remove(0);
            json_output = true;
        } else if args[0] == "-e" || args[0] == "--ecs" {
            args.remove(0);
            ecs_output = true;
        } else if args[0] == "-s" || args[0] == "--swap" {
            args.remove(0);
            if args.is_empty() {
                eprintln!("Error: -s requires a swap file path (.rs)");
                std::process::exit(1);
            }
            swap_file = Some(args.remove(0));
        } else if args[0] == "--strip" {
            args.remove(0);
            strip_signature = true;
        } else if args[0] == "--ndump" || args[0] == "-ndump" {
            args.remove(0);
            ndump = true;
        } else if args[0] == "--swapquickstart" {
            println!("[mt] Generating swap_quickstart.rs...");
            let content = include_str!("../examples/swap.rs");
            if let Err(e) = fs::write("swap_quickstart.rs", content) {
                eprintln!("[mt] Error writing file: {}", e);
                std::process::exit(1);
            }
            println!("[mt] Successfully created swap_quickstart.rs");
            println!("[mt] You can now edit it and run: mt -s swap_quickstart.rs <command>");
            std::process::exit(0);
        } else if args[0] == "-h" || args[0] == "-help" || args[0] == "--help" {
            print_help();
            std::process::exit(0);
        } else if args[0].starts_with("-") {
            eprintln!("Unknown argument: {}", args[0]);
            eprintln!("Use -h or --help for usage information.");
            std::process::exit(1);
        } else {
            break;
        }
    }

    if args.is_empty() {
        eprintln!("Error: No command specified to trace.");
        std::process::exit(1);
    }

    let mut cmd_name = args.remove(0);
    let mut zombie_root = None;

    if strip_signature {
        let (exec_path, root_path) = strip_and_copy(&cmd_name);
        cmd_name = exec_path;
        zombie_root = Some(root_path);
    } else {
        check_sip_and_codesign(&cmd_name);
    }
    
    let mut compiled_swap_path = None;
    if let Some(swap_script) = swap_file {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros();
        let out_path = format!("/tmp/mtrace_swap_{}.dylib", ts);
        
        println!("[mt] Compiling swap script: {} -> {}", swap_script, out_path);
        let status = Command::new("rustc")
            .args(["--crate-type", "cdylib", &swap_script, "-o", &out_path])
            .status()?;
            
        if !status.success() {
            eprintln!("[mt] Error: Failed to compile swap file. Check your Rust code!");
            std::process::exit(1);
        }
        compiled_swap_path = Some(out_path);
    }

    let mut cmd = Command::new(&cmd_name);
    cmd.args(&args);
    cmd.env("DYLD_INSERT_LIBRARIES", &dylib_path);
    
    if let Some(ref path) = compiled_swap_path {
        cmd.env("MTRACE_SWAP_DYLIB", path);
    }

    if let Some(out) = output_file {
        cmd.env("MTRACE_OUTPUT", out);
    }
    if let Some(filter) = trace_filter {
        cmd.env("MTRACE_FILTER", filter);
    }
    if json_output {
        cmd.env("MTRACE_JSON", "1");
    }
    if ecs_output {
        cmd.env("MTRACE_ECS", "1");
    }
    if ndump {
        cmd.env("MTRACE_NDUMP", "1");
    }

    let mut child = cmd.spawn()?;
    CHILD_PID.store(child.id(), Ordering::Relaxed);

    unsafe {
        libc::signal(libc::SIGINT, handle_signal as usize);
        libc::signal(libc::SIGTERM, handle_signal as usize);
    }

    let status = child.wait()?;
    CHILD_PID.store(0, Ordering::Relaxed);

    if status.success() {
        println!("[mt] Command '{}' finished successfully!", cmd_name);
    } else {
        println!("[mt] Command '{}' exited with status: {}", cmd_name, status);
    }
    
    if let Some(root) = zombie_root {
        println!("[mt] Cleaning up zombie copy at {}...", root);
        let _ = Command::new("rm").args(["-rf", &root]).status();
    }

    if let Some(path) = compiled_swap_path {
        let _ = std::fs::remove_file(path);
    }

    Ok(())
}
