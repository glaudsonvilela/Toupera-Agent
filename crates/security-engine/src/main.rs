use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use security_engine::{
    CachingProvider, CompanionAnalysis, DeepSeekProvider, RateCard, analyze_economic_functions,
    compare_with_solc_contract, empty_review_report, finalize_evidence_bundle,
    import_gigahorse_tac, import_heimdall_abi, judge_foundry_case,
    load_or_build_context_package_with_evm_version, prepare_evidence_bundle, prepare_foundry_cases,
    prepare_review_plan, reconstruct_bytecode_hex, reconstruct_source_file, reject_foundry_case,
    run_review,
};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(source) = args.next() else {
        eprintln!(
            "usage: security-engine <source.sol> [solc-path] | --economic-candidates <source.sol> [solc-path] [slither.json] | --context-package <source.sol> <private-cache-dir> <solc-path> [--slither <json>] [--bytecode <runtime.hex>] [--evm-version <solc-evm-version>] | --llm-plan <context.json> | --llm-review <context.json> --provider deepseek --allow-external --review-cache <private-dir> [--model <name>] [--max-candidates <n>] [--rate-card <json>] | --foundry-plan <context.json> <review.json> <source.sol> <private-output-dir> | --foundry-run <case-dir> <forge-bin> <solc-bin> | --foundry-reject <case-dir> | --evidence-plan <case-dir> <private-output-dir> | --evidence-finalize <bundle-dir> | --bytecode <runtime.hex> | --bytecode-companions <runtime.hex> <heimdall-abi.json> <gigahorse-contract.tac> | --compare-bytecode <runtime.hex> <solc.json> <source-name> <contract-name>"
        );
        return ExitCode::FAILURE;
    };
    if source == "--evidence-plan" || source == "--evidence-finalize" {
        let arguments: Vec<_> = args.collect();
        let result = if source == "--evidence-plan" {
            if arguments.len() != 2 {
                eprintln!("usage: security-engine --evidence-plan <case-dir> <private-output-dir>");
                return ExitCode::FAILURE;
            }
            prepare_evidence_bundle(Path::new(&arguments[0]), Path::new(&arguments[1])).and_then(
                |manifest| serde_json::to_value(manifest).map_err(|error| error.to_string()),
            )
        } else {
            if arguments.len() != 1 {
                eprintln!("usage: security-engine --evidence-finalize <bundle-dir>");
                return ExitCode::FAILURE;
            }
            finalize_evidence_bundle(Path::new(&arguments[0]))
                .and_then(|ready| serde_json::to_value(ready).map_err(|error| error.to_string()))
        };
        return match result {
            Ok(value) => {
                if let Err(error) = serde_json::to_writer(std::io::stdout(), &value) {
                    eprintln!("cannot write evidence output: {error}");
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
    if source == "--foundry-plan" || source == "--foundry-run" || source == "--foundry-reject" {
        let arguments: Vec<_> = args.collect();
        let result = if source == "--foundry-plan" {
            if arguments.len() != 4 {
                eprintln!(
                    "usage: security-engine --foundry-plan <context.json> <review.json> <source.sol> <private-output-dir>"
                );
                return ExitCode::FAILURE;
            }
            let read_json = |path: &str| -> Result<serde_json::Value, String> {
                let bytes =
                    fs::read(path).map_err(|error| format!("cannot read {path}: {error}"))?;
                serde_json::from_slice(&bytes).map_err(|error| format!("invalid {path}: {error}"))
            };
            read_json(&arguments[0]).and_then(|context| {
                read_json(&arguments[1]).and_then(|review| {
                    prepare_foundry_cases(
                        &context,
                        &review,
                        Path::new(&arguments[2]),
                        Path::new(&arguments[3]),
                    )
                    .and_then(|plan| serde_json::to_value(plan).map_err(|error| error.to_string()))
                })
            })
        } else if source == "--foundry-run" {
            if arguments.len() != 3 {
                eprintln!("usage: security-engine --foundry-run <case-dir> <forge-bin> <solc-bin>");
                return ExitCode::FAILURE;
            }
            judge_foundry_case(
                Path::new(&arguments[0]),
                Path::new(&arguments[1]),
                Path::new(&arguments[2]),
            )
            .and_then(|report| serde_json::to_value(report).map_err(|error| error.to_string()))
        } else {
            if arguments.len() != 1 {
                eprintln!("usage: security-engine --foundry-reject <case-dir>");
                return ExitCode::FAILURE;
            }
            reject_foundry_case(Path::new(&arguments[0]))
                .and_then(|report| serde_json::to_value(report).map_err(|error| error.to_string()))
        };
        return match result {
            Ok(value) => {
                if let Err(error) = serde_json::to_writer(std::io::stdout(), &value) {
                    eprintln!("cannot write Foundry output: {error}");
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
    if source == "--context-package" {
        let arguments: Vec<_> = args.collect();
        if arguments.len() < 3 || (arguments.len() - 3) % 2 != 0 {
            eprintln!(
                "usage: security-engine --context-package <source.sol> <private-cache-dir> <solc-path> [--slither <json>] [--bytecode <runtime.hex>] [--evm-version <solc-evm-version>]"
            );
            return ExitCode::FAILURE;
        }
        let mut slither_path = None;
        let mut runtime_path = None;
        let mut evm_version = None;
        for pair in arguments[3..].chunks_exact(2) {
            match pair[0].as_str() {
                "--slither" if slither_path.is_none() => slither_path = Some(&pair[1]),
                "--bytecode" if runtime_path.is_none() => runtime_path = Some(&pair[1]),
                "--evm-version" if evm_version.is_none() => evm_version = Some(&pair[1]),
                _ => {
                    eprintln!("unknown or repeated context option: {}", pair[0]);
                    return ExitCode::FAILURE;
                }
            }
        }
        let result = slither_path
            .map(|path| {
                fs::read_to_string(path).map_err(|error| format!("cannot read {path}: {error}"))
            })
            .transpose()
            .and_then(|slither| {
                runtime_path
                    .map(|path| {
                        fs::read_to_string(path)
                            .map_err(|error| format!("cannot read {path}: {error}"))
                    })
                    .transpose()
                    .and_then(|runtime| {
                        load_or_build_context_package_with_evm_version(
                            Path::new(&arguments[0]),
                            Path::new(&arguments[2]),
                            Path::new(&arguments[1]),
                            slither.as_deref(),
                            runtime.as_deref(),
                            evm_version.map(String::as_str),
                        )
                    })
            });
        return match result {
            Ok(package) => {
                if let Err(error) = serde_json::to_writer(std::io::stdout(), &package) {
                    eprintln!("cannot write context package: {error}");
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
    if source == "--llm-plan" || source == "--llm-review" {
        let arguments: Vec<_> = args.collect();
        if arguments.is_empty() {
            eprintln!("usage: security-engine {source} <context.json> [review options]");
            return ExitCode::FAILURE;
        }
        let input = match fs::read_to_string(&arguments[0])
            .map_err(|error| format!("cannot read {}: {error}", arguments[0]))
            .and_then(|text| {
                serde_json::from_str::<serde_json::Value>(&text)
                    .map_err(|error| format!("invalid M5 package JSON: {error}"))
            })
            .and_then(|value| prepare_review_plan(&value))
        {
            Ok(plan) => plan,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::FAILURE;
            }
        };
        if source == "--llm-plan" {
            if arguments.len() != 1 {
                eprintln!("usage: security-engine --llm-plan <context.json>");
                return ExitCode::FAILURE;
            }
            if let Err(error) = serde_json::to_writer(std::io::stdout(), &input) {
                eprintln!("cannot write LLM plan: {error}");
                return ExitCode::FAILURE;
            }
            println!();
            return ExitCode::SUCCESS;
        }
        let mut provider_name = None;
        let mut model = "deepseek-flash".to_owned();
        let mut max_candidates = 5_usize;
        let mut rate_card_path = None;
        let mut review_cache_path = None;
        let mut allow_external = false;
        let mut index = 1;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--allow-external" if !allow_external => {
                    allow_external = true;
                    index += 1;
                }
                "--provider" if provider_name.is_none() && index + 1 < arguments.len() => {
                    provider_name = Some(arguments[index + 1].as_str());
                    index += 2;
                }
                "--model" if index + 1 < arguments.len() => {
                    model = arguments[index + 1].clone();
                    index += 2;
                }
                "--max-candidates" if index + 1 < arguments.len() => {
                    max_candidates = match arguments[index + 1].parse::<usize>() {
                        Ok(value) if (1..=20).contains(&value) => value,
                        _ => {
                            eprintln!("--max-candidates must be 1..=20");
                            return ExitCode::FAILURE;
                        }
                    };
                    index += 2;
                }
                "--rate-card" if rate_card_path.is_none() && index + 1 < arguments.len() => {
                    rate_card_path = Some(arguments[index + 1].as_str());
                    index += 2;
                }
                "--review-cache" if review_cache_path.is_none() && index + 1 < arguments.len() => {
                    review_cache_path = Some(arguments[index + 1].as_str());
                    index += 2;
                }
                _ => {
                    eprintln!("unknown or repeated LLM option: {}", arguments[index]);
                    return ExitCode::FAILURE;
                }
            }
        }
        if input.requests.is_empty() {
            let report = empty_review_report(&input);
            if let Err(error) = serde_json::to_writer(std::io::stdout(), &report) {
                eprintln!("cannot write empty LLM report: {error}");
                return ExitCode::FAILURE;
            }
            println!();
            return ExitCode::SUCCESS;
        }
        if provider_name != Some("deepseek") || !allow_external {
            eprintln!("external LLM review requires --provider deepseek --allow-external");
            return ExitCode::FAILURE;
        }
        let Some(review_cache_path) = review_cache_path else {
            eprintln!("external LLM review requires --review-cache <private-dir>");
            return ExitCode::FAILURE;
        };
        let rate_card = match rate_card_path
            .map(|path| {
                fs::read_to_string(path)
                    .map_err(|error| format!("cannot read rate card {path}: {error}"))
                    .and_then(|text| {
                        serde_json::from_str::<RateCard>(&text)
                            .map_err(|error| format!("invalid rate card: {error}"))
                    })
                    .and_then(|card| {
                        card.validate()?;
                        Ok(card)
                    })
            })
            .transpose()
        {
            Ok(card) => card,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::FAILURE;
            }
        };
        let key = match env::var("TOUPERA_DEEPSEEK_API_KEY") {
            Ok(key) if !key.trim().is_empty() => key,
            _ => {
                eprintln!("TOUPERA_DEEPSEEK_API_KEY is not set");
                return ExitCode::FAILURE;
            }
        };
        let provider = match DeepSeekProvider::new(key, model) {
            Ok(provider) => provider,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::FAILURE;
            }
        };
        let provider = match CachingProvider::new(provider, Path::new(review_cache_path)) {
            Ok(provider) => provider,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::FAILURE;
            }
        };
        let report = run_review(&input, &provider, max_candidates, rate_card.as_ref());
        if let Err(error) = serde_json::to_writer(std::io::stdout(), &report) {
            eprintln!("cannot write LLM report: {error}");
            return ExitCode::FAILURE;
        }
        println!();
        return if report.stopped_after_error {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
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
