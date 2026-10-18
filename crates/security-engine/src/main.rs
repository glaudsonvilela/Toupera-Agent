use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use security_engine::{
    CompanionAnalysis, compare_with_solc_contract, import_gigahorse_tac, import_heimdall_abi,
    reconstruct_bytecode_hex, reconstruct_source_file,
};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(source) = args.next() else {
        eprintln!(
            "usage: security-engine <source.sol> [solc-path] | --bytecode <runtime.hex> | --bytecode-companions <runtime.hex> <heimdall-abi.json> <gigahorse-contract.tac> | --compare-bytecode <runtime.hex> <solc.json> <source-name> <contract-name>"
        );
        return ExitCode::FAILURE;
    };
    if source == "--bytecode" {
        let Some(path) = args.next() else {
            eprintln!("usage: security-engine --bytecode <runtime.hex>");
            return ExitCode::FAILURE;
        };
        if args.next().is_some() {
            eprintln!("usage: security-engine --bytecode <runtime.hex>");
            return ExitCode::FAILURE;
        }
        let result = fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {path}: {error}"))
            .and_then(|hex| reconstruct_bytecode_hex(&path, &hex));
        return match result {
            Ok(analysis) => {
                if let Err(error) = serde_json::to_writer(std::io::stdout(), &analysis) {
                    eprintln!("cannot write bytecode IR: {error}");
                    return ExitCode::FAILURE;
                }
                println!();
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        };
    }
    if source == "--bytecode-companions" {
        let arguments: Vec<_> = args.collect();
        if arguments.len() != 3 {
            eprintln!(
                "usage: security-engine --bytecode-companions <runtime.hex> <heimdall-abi.json> <gigahorse-contract.tac>"
            );
            return ExitCode::FAILURE;
        }
        let result = fs::read_to_string(&arguments[0])
            .map_err(|error| format!("cannot read {}: {error}", arguments[0]))
            .and_then(|hex| reconstruct_bytecode_hex(&arguments[0], &hex))
            .and_then(|native| {
                let abi = fs::read_to_string(&arguments[1])
                    .map_err(|error| format!("cannot read {}: {error}", arguments[1]))?;
                let tac = fs::read_to_string(&arguments[2])
                    .map_err(|error| format!("cannot read {}: {error}", arguments[2]))?;
                let heimdall = import_heimdall_abi(&native, &arguments[1], &abi)?;
                let gigahorse = import_gigahorse_tac(&native, &arguments[2], &tac)?;
                Ok(CompanionAnalysis {
                    native,
                    heimdall,
                    gigahorse,
                })
            });
        return match result {
            Ok(analysis) => {
                if let Err(error) = serde_json::to_writer(std::io::stdout(), &analysis) {
                    eprintln!("cannot write companion analysis: {error}");
                    return ExitCode::FAILURE;
                }
                println!();
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        };
    }
    if source == "--compare-bytecode" {
        let arguments: Vec<_> = args.collect();
        if arguments.len() != 4 {
            eprintln!(
                "usage: security-engine --compare-bytecode <runtime.hex> <solc.json> <source-name> <contract-name>"
            );
            return ExitCode::FAILURE;
        }
        let result = fs::read_to_string(&arguments[0])
            .map_err(|error| format!("cannot read {}: {error}", arguments[0]))
            .and_then(|hex| reconstruct_bytecode_hex(&arguments[0], &hex))
            .and_then(|analysis| {
                fs::read_to_string(&arguments[1])
                    .map_err(|error| format!("cannot read {}: {error}", arguments[1]))
                    .and_then(|compiler_json| {
                        compare_with_solc_contract(
                            &analysis,
                            &compiler_json,
                            &arguments[2],
                            &arguments[3],
                        )
                    })
            });
        return match result {
            Ok(comparison) => {
                if let Err(error) = serde_json::to_writer(std::io::stdout(), &comparison) {
                    eprintln!("cannot write comparison: {error}");
                    return ExitCode::FAILURE;
                }
                println!();
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        };
    }
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
