//! Standalone, read-only forensic sieve for a pinned wCOP job.
#[path = "../wcop_forensic_sift.rs"]
mod wcop_forensic_sift;

use std::env;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() != 1 {
        eprintln!("usage: wcop-forensic-sift <job-dir>");
        return ExitCode::FAILURE;
    }
    match wcop_forensic_sift::sieve_wcop_forensic(Path::new(&args[0])) {
        Ok(report) => {
            if serde_json::to_writer(std::io::stdout(), &report).is_err() {
                return ExitCode::FAILURE;
            }
            println!();
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
