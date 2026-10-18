use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use security_engine::{
    CompanionAnalysis, analyze_economic_functions, compare_with_solc_contract,
    import_gigahorse_tac, import_heimdall_abi, reconstruct_bytecode_hex, reconstruct_source_file,
};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(source) = args.next() else {
        eprintln!(
            "usage: security-engine <source.sol> [solc-path] | --economic-candidates <source.sol> [solc-path] [slither.json] | --bytecode <runtime.hex> | --bytecode-companions <runtime.hex> <heimdall-abi.json> <gigahorse-contract.tac> | --compare-bytecode <runtime.hex> <solc.json> <source-name> <contract-name>"
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
    if source == "--economic-candidates" {
        let arguments: Vec<_> = args.collect();
        if arguments.is_empty() || arguments.len() > 3 {
            eprintln!(
                "usage: security-engine --economic-candidates <source.sol> [solc-path] [slither.json]"
            );
            return ExitCode::FAILURE;
        }
        let compiler = arguments
            .get(1)
            .cloned()
            .or_else(|| env::var("SOLC_BIN").ok())
            .unwrap_or_else(|| "solc".to_owned());
        let result = fs::read_to_string(&arguments[0])
            .map_err(|error| format!("cannot read {}: {error}", arguments[0]))
            .and_then(|text| {
                let functions =
                    reconstruct_source_file(Path::new(&arguments[0]), Path::new(&compiler))?;
                let source_name = Path::new(&arguments[0])
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or("source path has no UTF-8 file name")?;
                let slither = arguments
                    .get(2)
                    .map(|path| {
                        fs::read_to_string(path)
                            .map_err(|error| format!("cannot read {path}: {error}"))
                    })
                    .transpose()?;
                analyze_economic_functions(&functions, source_name, &text, slither.as_deref())
            });
        return match result {
            Ok(report) => {
                if let Err(error) = serde_json::to_writer(std::io::stdout(), &report) {
                    eprintln!("cannot write economic candidates: {error}");
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
