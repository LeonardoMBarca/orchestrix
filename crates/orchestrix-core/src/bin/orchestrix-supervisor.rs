//! Trusted local Job keeper. No provider protocol or arbitrary shell entry.
fn main() {
    if orchestrix_core::recovery_guardian::entry(std::env::args_os().skip(1).collect()).is_err() {
        std::process::exit(1);
    }
}
