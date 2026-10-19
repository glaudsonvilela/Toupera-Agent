use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn bytes_at(job: &Path, name: &str, max: u64) -> Result<Vec<u8>, String> {
    let path = job.join(name);
    let meta = fs::symlink_metadata(&path).map_err(|e| format!("{name}: {e}"))?;
    if !meta.file_type().is_file() || meta.len() > max {
        return Err(format!("{name}: arquivo inválido ou acima do limite"));
    }
    fs::read(path).map_err(|e| format!("{name}: {e}"))
}

fn json_at(job: &Path, name: &str, max: u64) -> Result<Value, String> {
    serde_json::from_slice(&bytes_at(job, name, max)?)
        .map_err(|e| format!("{name}: JSON inválido: {e}"))
}

fn code_bytes(value: &str) -> Result<Vec<u8>, String> {
    let value = value.strip_prefix("0x").unwrap_or(value);
    if value.is_empty() || value.len() % 2 != 0 || value.len() > 200_000 {
        return Err("runtime hexadecimal fora dos limites".into());
    }
    hex::decode(value).map_err(|_| "runtime hexadecimal inválido".into())
}

fn valid_hex(value: &str, digits: usize) -> bool {
    value.len() == digits + 2
        && value.starts_with("0x")
        && value[2..].bytes().all(|b| b.is_ascii_hexdigit())
}

fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 240
        && name.ends_with(".sol")
        && !name.starts_with('/')
        && !name.contains('\\')
        && !name.contains('\0')
        && name.split('/').all(|p| !matches!(p, "" | "." | ".."))
}

fn walk_ast(
    item: &Value,
    file_id: i64,
    source_len: usize,
    depth: usize,
    visited: &mut usize,
    ids: &mut HashSet<i64>,
) -> Result<(), String> {
    if depth > 256 || *visited > 1_000_000 {
        return Err("AST acima do limite de profundidade ou nós".into());
    }
    match item {
        Value::Object(map) => {
            if map.contains_key("nodeType") {
                *visited += 1;
                if let Some(id) = item["id"].as_i64() {
                    if !ids.insert(id) {
                        return Err("ID AST repetido".into());
                    }
                } else if !item["nodeType"].as_str().unwrap_or("").starts_with("Yul") {
                    return Err("nó Solidity sem ID".into());
                }
                let src = item["src"].as_str().ok_or("nó AST sem faixa")?;
                let parts: Vec<_> = src.split(':').collect();
                if parts.len() != 3 {
                    return Err("faixa AST inválida".into());
                }
                let start = parts[0]
                    .parse::<usize>()
                    .map_err(|_| "início AST inválido")?;
                let len = parts[1]
                    .parse::<usize>()
                    .map_err(|_| "tamanho AST inválido")?;
                let owner = parts[2]
                    .parse::<i64>()
                    .map_err(|_| "arquivo AST inválido")?;
                if owner != file_id || start.checked_add(len).is_none_or(|n| n > source_len) {
                    return Err("faixa AST fora do arquivo associado".into());
                }
            }
            for child in map.values() {
                walk_ast(child, file_id, source_len, depth + 1, visited, ids)?;
            }
        }
        Value::Array(items) => {
            for child in items {
                walk_ast(child, file_id, source_len, depth + 1, visited, ids)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_ast(input: &Value, output: &Value) -> Result<(String, String), String> {
    if input["language"].as_str() != Some("Solidity") {
        return Err("linguagem de entrada inesperada".into());
    }
    let sources = input["sources"].as_object().ok_or("fontes ausentes")?;
    let parsed = output["sources"]
        .as_object()
        .ok_or("AST ausente da compilação")?;
    if sources.is_empty() || sources.len() > 256 || sources.len() != parsed.len() {
        return Err("conjunto de fontes inconsistente".into());
    }
    if output["errors"]
        .as_array()
        .is_some_and(|errors| errors.iter().any(|e| e["severity"] == "error"))
    {
        return Err("compilador registrou erro".into());
    }
    let mut total = 0usize;
    let mut ids = HashSet::new();
    let mut file_ids = HashSet::new();
    let mut visited = 0usize;
    for (name, item) in sources {
        if !safe_name(name) {
            return Err("nome de fonte inválido".into());
        }
        let content = item["content"]
            .as_str()
            .ok_or("conteúdo de fonte ausente")?;
        let len = content.len();
        total += len;
        if len > 1_000_000 || total > 10_000_000 {
            return Err("fontes acima do limite".into());
        }
        let parsed_file = parsed.get(name).ok_or("fonte sem AST")?;
        let file_id = parsed_file["id"].as_i64().ok_or("ID de fonte ausente")?;
        if !file_ids.insert(file_id) {
            return Err("ID de arquivo repetido".into());
        }
        let ast = parsed_file.get("ast").ok_or("AST de fonte ausente")?;
        if ast["nodeType"] != "SourceUnit" {
            return Err("raiz AST inesperada".into());
        }
        if ast["absolutePath"].as_str() != Some(name) {
            return Err("caminho AST diferente da fonte".into());
        }
        walk_ast(ast, file_id, len, 0, &mut visited, &mut ids)?;
    }
    let selection = input["settings"]["outputSelection"]
        .as_object()
        .ok_or("seleção de compilação ausente")?;
    let (main_file, contracts) = selection
        .iter()
        .find(|(name, _)| name.as_str() != "*")
        .ok_or("contrato principal não selecionado")?;
    if !sources.contains_key(main_file) {
        return Err("contrato principal fora das fontes".into());
    }
    let main_name = contracts
        .as_object()
        .and_then(|v| v.keys().find(|k| !k.is_empty()))
        .ok_or("nome do contrato principal ausente")?;
    Ok((main_file.clone(), main_name.clone()))
}

fn source_binding(
    job: &Path,
    input: &Value,
    output: &Value,
    scope: &Value,
    runtime: &[u8],
) -> Result<String, String> {
    let (file, name) = validate_ast(input, output)?;
    for (name, item) in input["sources"].as_object().ok_or("fontes ausentes")? {
        let expected = item["content"]
            .as_str()
            .ok_or("conteúdo de fonte ausente")?;
        if bytes_at(&job.join("source"), name, 1_000_000)? != expected.as_bytes() {
            return Err("arquivo usado pelas ferramentas difere da compilação".into());
        }
    }
    let deployed = &output["contracts"][&file][&name]["evm"]["deployedBytecode"];
    let compiled = code_bytes(
        deployed["object"]
            .as_str()
            .ok_or("runtime compilado ausente")?,
    )?;
    if compiled.len() != runtime.len() {
        return Err("runtime compilado tem outro tamanho".into());
    }
    let binding = if compiled == runtime {
        "exact"
    } else {
        let refs = deployed["immutableReferences"]
            .as_object()
            .ok_or("referências imutáveis ausentes")?;
        let mut covered = vec![false; runtime.len()];
        for positions in refs.values() {
            for part in positions.as_array().ok_or("lista imutável inválida")? {
                let start = part["start"].as_u64().ok_or("posição imutável inválida")? as usize;
                let len = part["length"].as_u64().ok_or("tamanho imutável inválido")? as usize;
                if len == 0 || start.checked_add(len).is_none_or(|end| end > runtime.len()) {
                    return Err("faixa imutável fora do runtime".into());
                }
                for slot in &mut covered[start..start + len] {
                    if *slot {
                        return Err("faixas imutáveis sobrepostas".into());
                    }
                    *slot = true;
                }
            }
        }
        if compiled
            .iter()
            .zip(runtime)
            .enumerate()
            .any(|(i, (a, b))| a != b && !covered[i])
        {
            return Err("runtime diverge fora das faixas imutáveis".into());
        }
        "masked_immutables"
    };
    let expected = if binding == "exact" {
        "idêntico"
    } else {
        "confere sem campos imutáveis"
    };
    if scope["compiler"]["state"].as_str() != Some(expected) {
        return Err("resumo de compilação contradiz os bytes".into());
    }
    Ok(binding.into())
}

fn claim(id: &str, title: &str, state: &str, reason: &str, evidence: &[&str]) -> Value {
    json!({"id": id, "title": title, "state": state, "reason": reason, "evidence": evidence})
}

pub fn guard_wcop_job(job: &Path) -> Result<Value, String> {
    let manifest = json_at(job, "job-manifest.json", 1_000_000)?;
    let artifacts = manifest["artifacts"]
        .as_object()
        .ok_or("manifesto sem artefatos")?;
    let address = manifest["address"].as_str().ok_or("endereço ausente")?;
    let block_hash = manifest["block_hash"]
        .as_str()
        .ok_or("hash do bloco ausente")?;
    let block = manifest["block_number"].as_u64().ok_or("bloco ausente")?;
    if manifest["format"] != 1
        || manifest["chain_id"] != 1
        || !valid_hex(address, 40)
        || !valid_hex(block_hash, 64)
        || block == 0
    {
        return Err("identidade do trabalho inválida".into());
    }
    let implementation = manifest["implementation"].as_str();
    if implementation.is_some_and(|v| !valid_hex(v, 40)) {
        return Err("endereço da implementação inválido".into());
    }
    let slot_word = manifest["eip1967_slot_word"]
        .as_str()
        .ok_or("slot EIP-1967 não registrado")?;
    if !valid_hex(slot_word, 64)
        || implementation
            .is_some_and(|address| !slot_word[26..].eq_ignore_ascii_case(&address[2..]))
    {
        return Err("slot EIP-1967 não corresponde à implementação".into());
    }
    let allowed = [
        "smart.md",
        "runtime.hex",
        "implementation.hex",
        "engine.json",
        "engine-impl.json",
        "compiler-input.json",
        "compiler-output.json",
        "scope.json",
        "history.json",
        "roles.json",
        "tools.json",
        "trace.json",
        "trace-reconcile.json",
        "scenarios.json",
        "coverage.json",
        "adaptive.json",
        "adaptive-state.json",
        "adaptive-trace.json",
        "adaptive-reconcile.json",
        "path-map.json",
    ];
    let mut reasons = Vec::<String>::new();
    let mut intact = HashSet::new();
    for (name, recorded) in artifacts {
        if !allowed.contains(&name.as_str()) {
            reasons.push(format!("artefato inesperado: {name}"));
            continue;
        }
        match bytes_at(job, name, 25_000_000) {
            Ok(data)
                if recorded["bytes"].as_u64() == Some(data.len() as u64)
                    && recorded["sha256"].as_str() == Some(digest(&data).as_str()) =>
            {
                intact.insert(name.as_str());
            }
            _ => reasons.push(format!("{name}: hash ou tamanho não confere")),
        }
    }
    let main_name = if implementation.is_some() {
        "implementation.hex"
    } else {
        "runtime.hex"
    };
    let engine_name = if implementation.is_some() {
        "engine-impl.json"
    } else {
        "engine.json"
    };
    let required = ["smart.md", "runtime.hex", main_name, engine_name];
    for name in required {
        if !intact.contains(name) {
            reasons.push(format!("{name}: entrada obrigatória ausente ou alterada"));
        }
    }
    let mut identity_ok = reasons.is_empty();
    if intact.contains("smart.md") {
        let text = String::from_utf8(bytes_at(job, "smart.md", 20_000)?)
            .map_err(|_| "smart.md não é UTF-8")?;
        if !text
            .lines()
            .any(|line| line == format!("- Endereço: {address}"))
            || !text.lines().any(|line| line == "- Rede: Ethereum (1)")
        {
            identity_ok = false;
            reasons.push("entrada do painel não corresponde ao manifesto".into());
        }
    }
    let engine_binary_ok = fs::read(std::env::current_exe().map_err(|e| e.to_string())?)
        .map(|bytes| manifest["engine_binary_sha256"].as_str() == Some(digest(&bytes).as_str()))
        .unwrap_or(false);
    if !engine_binary_ok {
        identity_ok = false;
        reasons.push("versão binária do motor difere do manifesto".into());
    }
    let solc_binary_ok = fs::read("/srv/toupera-agent/tools/solc-0.8.27")
        .map(|bytes| manifest["solc_binary_sha256"].as_str() == Some(digest(&bytes).as_str()))
        .unwrap_or(false);
    if !solc_binary_ok {
        reasons.push("versão binária do compilador difere do manifesto".into());
    }
    let proxy = if intact.contains("runtime.hex") {
        String::from_utf8(bytes_at(job, "runtime.hex", 200_000)?)
            .ok()
            .and_then(|v| code_bytes(v.trim()).ok())
    } else {
        None
    };
    let proxy_engine = if intact.contains("engine.json") {
        json_at(job, "engine.json", 25_000_000).ok()
    } else {
        None
    };
    let proxy_ok = proxy
        .as_ref()
        .zip(proxy_engine.as_ref())
        .is_some_and(|(raw, engine)| {
            engine["normalized_hex"]
                .as_str()
                .and_then(|v| code_bytes(v).ok())
                .is_some_and(|code| raw.starts_with(&code) && code.len() + 128 >= raw.len())
        });
    if !proxy_ok {
        identity_ok = false;
        reasons.push("mapa do proxy não corresponde ao runtime fixado".into());
    }
    let runtime = if intact.contains(main_name) {
        let text = String::from_utf8(bytes_at(job, main_name, 200_000)?)
            .map_err(|_| "runtime não é UTF-8")?;
        code_bytes(text.trim())?
    } else {
        Vec::new()
    };
    let engine = if intact.contains(engine_name) {
        Some(json_at(job, engine_name, 25_000_000)?)
    } else {
        None
    };
    if let Some(engine) = &engine {
        let normalized = engine["normalized_hex"].as_str().unwrap_or("");
        match code_bytes(normalized) {
            Ok(code) if runtime.starts_with(&code) && code.len() + 128 >= runtime.len() => {}
            _ => {
                identity_ok = false;
                reasons.push("mapa Rust não corresponde ao runtime fixado".into());
            }
        }
    }
    let mut binding = "unavailable".to_string();
    if identity_ok
        && solc_binary_ok
        && ["compiler-input.json", "compiler-output.json", "scope.json"]
            .iter()
            .all(|n| intact.contains(n))
    {
        let input = json_at(job, "compiler-input.json", 15_000_000)?;
        let output = json_at(job, "compiler-output.json", 25_000_000)?;
        let scope = json_at(job, "scope.json", 5_000_000)?;
        match source_binding(job, &input, &output, &scope, &runtime) {
            Ok(value) => binding = value,
            Err(reason) => {
                binding = "diverged".into();
                reasons.push(format!("fonte: {reason}"));
            }
        }
    } else if identity_ok {
        reasons.push("fonte ou AST indisponível; leitura direta do bytecode permanece".into());
    }
    let linked = matches!(binding.as_str(), "exact" | "masked_immutables");
    let mut claims = Vec::new();
    claims.push(claim(
        "bytecode",
        "Estrutura do bytecode",
        if identity_ok {
            "OBSERVADO"
        } else {
            "INCONCLUSIVO"
        },
        if identity_ok {
            "runtime e mapa Rust vinculados por bytes"
        } else {
            "identidade incompleta"
        },
        &[main_name, engine_name],
    ));
    claims.push(claim(
        "source",
        "Relação entre fonte e runtime",
        if !identity_ok || !linked {
            "INCONCLUSIVO"
        } else if binding == "exact" {
            "OBSERVADO"
        } else {
            "RELACIONADO_COM_RESSALVA"
        },
        match binding.as_str() {
            "exact" => "compilação reproduz os bytes do runtime",
            "masked_immutables" => {
                "diferenças restritas aos campos imutáveis declarados; valores não conferidos"
            }
            "diverged" => "AST, fonte ou runtime incompatível",
            _ => "fonte validada não disponível",
        },
        &[
            "compiler-input.json",
            "compiler-output.json",
            "scope.json",
            main_name,
        ],
    ));
    if intact.contains("history.json") {
        let history = json_at(job, "history.json", 10_000_000)?;
        let valid = identity_ok
            && history["address"].as_str() == Some(address)
            && history["through_block"].as_u64() == Some(block)
            && history["complete_event_range"] == true
            && history["supply_matches_events"] == true;
        claims.push(claim(
            "events",
            "Eventos e oferta no bloco",
            if valid { "OBSERVADO" } else { "INCONCLUSIVO" },
            if valid {
                "eventos recolhidos conferem com oferta lida no bloco"
            } else {
                "histórico incompleto, divergente ou de outro bloco"
            },
            &["history.json"],
        ));
    }
    if intact.contains("trace-reconcile.json") && intact.contains("trace.json") {
        let trace = json_at(job, "trace-reconcile.json", 10_000_000)?;
        let raw = json_at(job, "trace.json", 10_000_000)?;
        let hash = engine
            .as_ref()
            .and_then(|e| e["normalized_hex"].as_str())
            .and_then(|v| code_bytes(v).ok())
            .map(|v| digest(&v));
        let valid = identity_ok
            && trace["contract"].as_str() == Some(address)
            && raw["contract"].as_str() == Some(address)
            && trace["block"].as_u64() == Some(block)
            && raw["block"].as_u64() == Some(block)
            && raw["implementation"].as_str() == implementation
            && trace["trace_errors"].as_u64() == Some(0)
            && trace["engine_code_sha256"].as_str() == hash.as_deref()
            && raw["engine_code_sha256"].as_str() == hash.as_deref();
        claims.push(claim(
            "paths",
            "Caminhos locais do runtime",
            if valid { "OBSERVADO" } else { "INCONCLUSIVO" },
            if valid {
                "PCs ligados ao código fixado; alcance restrito às chamadas locais"
            } else {
                "identidade ou leitura dos caminhos não confere"
            },
            &["trace.json", "trace-reconcile.json", engine_name],
        ));
    }
    if intact.contains("path-map.json") {
        let map = json_at(job, "path-map.json", 5_000_000)?;
        let code_hash = engine.as_ref()
            .and_then(|value| value["normalized_hex"].as_str())
            .and_then(|value| code_bytes(value).ok())
            .map(|value| digest(&value));
        let compiler_hash = bytes_at(job, "compiler-input.json", 15_000_000)
            .ok().map(|value| digest(&value));
        let source_relation_ok = match binding.as_str() {
            "exact" => map["source_relation"] == "idêntico",
            "masked_immutables" => map["source_relation"] == "confere sem campos imutáveis",
            _ => false,
        };
        let valid = identity_ok && linked && source_relation_ok
            && map["compiler_input_sha256"].as_str() == compiler_hash.as_deref()
            && map["contract"].as_str() == Some(address)
            && map["block"].as_u64() == Some(block)
            && map["implementation"].as_str() == implementation
            && map["engine_code_sha256"].as_str() == code_hash.as_deref()
            && map["all_pcs"].as_u64() == engine.as_ref()
                .and_then(|value| value["instructions"].as_array())
                .map(|items| items.len() as u64)
            && map["observed_pcs"].as_u64().is_some_and(|count|
                count <= map["all_pcs"].as_u64().unwrap_or(0));
        claims.push(claim(
            "source_map", "Linhas ligadas ao runtime",
            if valid { "OBSERVADO" } else { "INCONCLUSIVO" },
            if valid { "locais do compilador ligados ao código e bloco fixados, com lacunas explícitas" }
            else { "vínculo do mapa incompleto" },
            &["path-map.json", "compiler-input.json", engine_name],
        ));
    }
    if intact.contains("scope.json") {
        let scope = json_at(job, "scope.json", 5_000_000)?;
        let count = scope["formal"]["properties_passed"].as_u64().unwrap_or(0);
        let valid = identity_ok && linked && scope["formal"]["state"] == "concluído" && count > 0;
        claims.push(claim(
            "properties",
            "Propriedades locais limitadas",
            if valid { "OBSERVADO" } else { "INCONCLUSIVO" },
            if valid {
                "propriedades concluídas somente sob premissas do harness local"
            } else {
                "propriedade ou vínculo da fonte não concluído"
            },
            &["scope.json"],
        ));
    }
    if ["adaptive.json", "adaptive-state.json", "adaptive-trace.json", "adaptive-reconcile.json"]
        .iter().all(|name| intact.contains(name)) {
        let summary = json_at(job, "adaptive.json", 1_000_000)?;
        let state = json_at(job, "adaptive-state.json", 5_000_000)?;
        let adaptive_trace = json_at(job, "adaptive-trace.json", 5_000_000)?;
        let review = json_at(job, "adaptive-reconcile.json", 5_000_000)?;
        let trace_bytes = bytes_at(job, "adaptive-trace.json", 5_000_000)?;
        let code_hash = engine.as_ref()
            .and_then(|value| value["normalized_hex"].as_str())
            .and_then(|value| code_bytes(value).ok())
            .map(|value| digest(&value));
        let valid = identity_ok
            && summary["contract"].as_str() == Some(address)
            && summary["block"].as_u64() == Some(block)
            && summary["engine_code_sha256"].as_str() == code_hash.as_deref()
            && summary["trace_sha256"].as_str() == Some(digest(&trace_bytes).as_str())
            && summary["attempts"].as_u64() == state["attempts"].as_array().map(|v| v.len() as u64)
            && summary["attempts"].as_u64().is_some_and(|v| v <= 40)
            && summary["combined_pcs"].as_u64() == state["visited_pcs"].as_array().map(|v| v.len() as u64)
            && adaptive_trace["engine_code_sha256"] == summary["engine_code_sha256"]
            && review["engine_code_sha256"] == summary["engine_code_sha256"]
            && review["trace_errors"].as_u64() == Some(0);
        claims.push(claim(
            "adaptive", "Ciclo local orientado por resultados",
            if valid { "OBSERVADO" } else { "INCONCLUSIVO" },
            if valid { "tentativas limitadas e caminhos novos ligados ao runtime fixado" }
            else { "ciclo ou identidade incompletos" },
            &["adaptive.json", "adaptive-state.json", "adaptive-trace.json", "adaptive-reconcile.json"],
        ));
    }
    if intact.contains("tools.json") {
        claims.push(claim(
            "signals",
            "Sinais das ferramentas",
            "HIPOTESE",
            "saídas brutas não confirmam uma falha sem reprodução e revisão",
            &["tools.json"],
        ));
    }
    let state = if !identity_ok {
        "inconclusivo"
    } else if linked {
        "escopo_vinculado"
    } else {
        "bytecode_parcial"
    };
    Ok(json!({
        "format": 1, "state": state, "identity_ok": identity_ok,
        "source_binding": binding, "chain_id": 1, "address": address,
        "block_number": block, "block_hash": block_hash,
        "implementation": implementation, "claims": claims, "reasons": reasons,
        "manifest_sha256": digest(&bytes_at(job, "job-manifest.json", 1_000_000)?),
        "interpretation": "decisão por afirmação; não é prova global nem confirmação automática"
    }))
}
