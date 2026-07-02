use std::process::Command;
use std::env;
use std::io;

fn main() -> io::Result<()> {
    // Dynamically find the dylib in the same directory as this executable
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

    // Very simple manual parsing for -o and -t
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
        } else if args[0].starts_with("-") {
            eprintln!("Unknown argument: {}", args[0]);
            eprintln!("Usage: mactrace [-o output.log] [-t open,read] <command> [args...]");
            std::process::exit(1);
        } else {
            break;
        }
    }

    if args.is_empty() {
        eprintln!("Usage: mactrace [-o output.log] [-t open,read] <command> [args...]");
        std::process::exit(1);
    }

    let cmd_name = args.remove(0);
    let mut cmd = Command::new(&cmd_name);
    cmd.args(&args);
    cmd.env("DYLD_INSERT_LIBRARIES", &dylib_path);

    if let Some(out) = output_file {
        cmd.env("MTRACE_OUTPUT", out);
    }
    if let Some(filter) = trace_filter {
        cmd.env("MTRACE_FILTER", filter);
    }

    let status = cmd.status()?;

    if status.success() {
        println!("[mactrace] Command '{}' finished successfully!", cmd_name);
    } else {
        println!("[mactrace] Command '{}' exited with status: {}", cmd_name, status);
    }

    Ok(())
}
