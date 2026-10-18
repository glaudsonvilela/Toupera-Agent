use security_engine::{
    ArithmeticExpression, BytecodePc, CallTarget, Confidence, ContractId, EconomicFunction,
    EconomicOperation, Evidence, EvidenceLocation, FunctionId, SourceSpan, StorageId, Uncertainty,
    UncertaintyKind,
};

const SOURCE: &str = include_str!("../../../tests/fixtures/VaultWithOracle.sol");
const FILE: &str = "tests/fixtures/VaultWithOracle.sol";

fn observed_after<T>(function_start: usize, needle: &str, value: T) -> Evidence<T> {
    let offset = SOURCE[function_start..]
        .find(needle)
        .expect("fixture must contain evidence");
    let start = function_start + offset;
    Evidence {
        value,
        location: EvidenceLocation::Source(
            SourceSpan::new(FILE, start, start + needle.len()).expect("nonempty source span"),
        ),
        confidence: Confidence::Direct,
    }
}

fn source_text<T>(evidence: &Evidence<T>) -> &str {
    let EvidenceLocation::Source(span) = &evidence.location else {
        panic!("fixture evidence must point to source");
    };
    assert_eq!(span.file(), FILE);
    span.slice(SOURCE).expect("source span must resolve")
}

#[test]
fn records_a_compact_economic_function_with_fixture_evidence() {
    let function_start = SOURCE.find("function deposit").expect("fixture function");
    let mut operation = observed_after(
        function_start,
        "function deposit",
        EconomicOperation::Deposit,
    );
    operation.confidence = Confidence::Inferred;
    let mut function = EconomicFunction::new(
        ContractId("VaultWithOracle".into()),
        FunctionId("deposit(uint256)".into()),
        operation,
    );

    function.reads.push(observed_after(
        function_start,
        "totalSupply == 0",
        StorageId("totalSupply".into()),
    ));
    function.reads.push(observed_after(
        function_start,
        "totalAssets;",
        StorageId("totalAssets".into()),
    ));
    function.writes.push(observed_after(
        function_start,
        "totalAssets += assets;",
        StorageId("totalAssets".into()),
    ));
    let oracle_call = observed_after(
        function_start,
        "oracle.latestPrice()",
        CallTarget("oracle.latestPrice".into()),
    );
    function.external_calls.push(oracle_call.clone());
    function.oracle_dependencies.push(oracle_call);
    function.arithmetic.push(observed_after(
        function_start,
        "assets * totalSupply / totalAssets",
        ArithmeticExpression("assets * totalSupply / totalAssets".into()),
    ));
    function.unresolved.push(Uncertainty {
        kind: UncertaintyKind::MissingInput,
        detail: "Fixture omits ERC20 asset transfers".into(),
        location: None,
    });

    assert_eq!(function.operation.confidence, Confidence::Inferred);
    assert_eq!(source_text(&function.operation), "function deposit");
    assert_eq!(source_text(&function.reads[0]), "totalSupply == 0");
    assert_eq!(source_text(&function.writes[0]), "totalAssets += assets;");
    assert_eq!(
        source_text(&function.oracle_dependencies[0]),
        "oracle.latestPrice()"
    );
    assert_eq!(
        source_text(&function.arithmetic[0]),
        "assets * totalSupply / totalAssets"
    );
    assert_eq!(function.unresolved[0].kind, UncertaintyKind::MissingInput);
}

#[test]
fn source_spans_reject_invalid_ranges_and_utf8_boundaries() {
    assert!(SourceSpan::new("", 0, 1).is_none());
    assert!(SourceSpan::new(FILE, 3, 3).is_none());
    assert!(SourceSpan::new(FILE, 4, 3).is_none());

    let text = "preço";
    let span = SourceSpan::new(FILE, 0, text.len()).unwrap();
    assert_eq!(span.slice(text), Some(text));
    assert_eq!(span.start_byte(), 0);
    assert_eq!(span.end_byte(), text.len());
    assert_eq!(span.slice("a"), None);

    let inside_multibyte = SourceSpan::new(FILE, 3, 4).unwrap();
    assert_eq!(inside_multibyte.slice(text), None);
}

#[test]
fn bytecode_location_can_identify_an_artifact_and_program_counter() {
    let location = EvidenceLocation::Bytecode(BytecodePc {
        artifact: "implementation-runtime".into(),
        pc: 42,
    });
    assert_eq!(
        location,
        EvidenceLocation::Bytecode(BytecodePc {
            artifact: "implementation-runtime".into(),
            pc: 42,
        })
    );
}
