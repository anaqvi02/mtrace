use std::process::Command;
use std::env::args;
use std::io;



fn main() -> io::Result<()> {

    let dylib_path = "/Users/alinaqvi/RustroverProjects/mtrace/target/debug/libmactrace_lib.dylib";

    if !std::path::Path::new(dylib_path).exists() {
        eprintln!("[mactrace] Error: Dylib not found at {}", dylib_path);
        std::process::exit(1);
    }

    let mut myargs: Vec<String> = args().skip(1).collect();
    if myargs.is_empty() {
        eprintln!("Usage: mactrace <command> [args...]");
        std::process::exit(1);
    }

    let cmdname = &myargs.remove(0);

    let status = Command::new(&cmdname)
        .args(&myargs)
        .env("DYLD_INSERT_LIBRARIES","/Users/alinaqvi/RustroverProjects/mtrace/target/debug/libmactrace_lib.dylib")
        .status()?;

    if status.success() {
        println!("[mactrace] Command {} {} finished!",cmdname,&myargs.join(" "));
    }
    Ok(())
}
