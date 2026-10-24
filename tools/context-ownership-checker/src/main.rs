use orchestrix_context_ownership_checker_v2::{
    describe, error, exit_code, inspect_current_directory,
};
use std::io::Write;
fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let value = if args.len() == 1 && args[0] == "--describe" {
        describe()
    } else if args.is_empty() {
        inspect_current_directory()
    } else {
        error("arguments-not-supported")
    };
    let code = if args.len() == 1 && args[0] == "--describe" {
        0
    } else {
        exit_code(&value)
    };
    if writeln!(std::io::stdout().lock(), "{value}").is_err() {
        std::process::exit(2);
    }
    std::process::exit(code);
}
