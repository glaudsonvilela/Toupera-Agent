use std::env;
use std::path::Path;
use std::process::ExitCode;

use security_engine::reconstruct_source_file;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(source) = args.next() else {
        eprintln!("usage: security-engine <source.sol> [solc-path]");
        return ExitCode::FAILURE;
    };
    let compiler = args
        .next()
        .or_else(|| env::var("SOLC_BIN").ok())
        .unwrap_or_else(|| "solc".to_owned());
    if args.next().is_some() {
        eprintln!("usage: security-engine <source.sol> [solc-path]");
        return ExitCode::FAILURE;
    }

    match reconstruct_source_file(Path::new(&source), Path::new(&compiler)) {
        Ok(functions) => {
            if let Err(error) = serde_json::to_writer(std::io::stdout(), &functions) {
                eprintln!("cannot write IR: {error}");
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
