use std::process::Command;
use std::env;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

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
    println!("  --swapquickstart       Download a template swap.rs file to the current directory");
    println!("  -h, --help             Print this help message and exit");
    println!("");
    println!("Example:");
    println!("  mtrace -t open,socket -j -o trace.json curl http://example.com");
}

fn main() -> io::Result<()> {
    let mut dylib_path = env::current_exe()?;
    dylib_path.set_file_name("libmactrace_lib.dylib");

    if !dylib_path.exists() {
        eprintln!("[mactrace] Error: Dylib not found at {}", dylib_path.display());
        eprintln!("[mactrace] Make sure you built the project with `cargo build`");
        std::process::exit(1);
    }

    let mut args: Vec<String> = env::args().skip(1).collect();
    let mut output_file = None;
    let mut trace_filter = None;
    let mut swap_file = None;
    let mut json_output = false;
    let mut ecs_output = false;

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
        } else if args[0] == "--swapquickstart" {
            println!("[mactrace] Downloading swap_quickstart.rs from GitHub...");
            let status = Command::new("curl")
                .args(["-sL", "https://raw.githubusercontent.com/anaqvi02/mtrace/main/examples/swap.rs", "-o", "swap_quickstart.rs"])
                .status();
            
            match status {
                Ok(s) if s.success() => {
                    println!("[mactrace] Successfully downloaded swap_quickstart.rs");
                    println!("[mactrace] You can now edit it and run: mt -s swap_quickstart.rs <command>");
                    std::process::exit(0);
                }
                _ => {
                    eprintln!("[mactrace] Error: Failed to download the quickstart file. Check your internet connection or URL.");
                    std::process::exit(1);
                }
            }
        } else if args[0] == "-h" || args[0] == "-help" || args[0] == "--help" {
            print_help();
            std::process::exit(0);
        } else if args[0].starts_with("-") {
            eprintln!("Unknown argument: {}", args[0]);
            eprintln!("Use -h or --help for usage information.");
            std::process::exit(1);
        }

        else if args[0].starts_with("--swapquickstart"){
            eprintln!("Creating quickstart swap file!");
        }

        else {
            break;
        }
    }

    if args.is_empty() {
        eprintln!("Error: No command specified to trace.");
        std::process::exit(1);
    }

    let cmd_name = args.remove(0);
    
    let mut compiled_swap_path = None;
    if let Some(swap_script) = swap_file {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros();
        let out_path = format!("/tmp/mtrace_swap_{}.dylib", ts);
        
        println!("[mactrace] Compiling swap script: {} -> {}", swap_script, out_path);
        let status = Command::new("rustc")
            .args(["--crate-type", "cdylib", &swap_script, "-o", &out_path])
            .status()?;
            
        if !status.success() {
            eprintln!("[mactrace] Error: Failed to compile swap file. Check your Rust code!");
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

    let status = cmd.status()?;

    if status.success() {
        println!("[mactrace] Command '{}' finished successfully!", cmd_name);
    } else {
        println!("[mactrace] Command '{}' exited with status: {}", cmd_name, status);
    }
    
    if let Some(path) = compiled_swap_path {
        let _ = std::fs::remove_file(path);
    }

    Ok(())
}
