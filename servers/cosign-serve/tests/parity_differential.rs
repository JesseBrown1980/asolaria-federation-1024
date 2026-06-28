use std::path::Path;

#[test]
#[ignore = "requires a path-patched Python daemon copy and temp ledgers"]
fn refuses_to_run_without_path_patched_daemon() {
    let daemon = std::env::var("ASOLARIA_COSIGN_PATCHED_DAEMON")
        .expect("path-patched daemon required; never point tests at live :4953 daemon");
    let py_ledger =
        std::env::var("ASOLARIA_COSIGN_DIFF_PY_LEDGER").expect("temp Python ledger path required");
    let rust_ledger = std::env::var("ASOLARIA_COSIGN_DIFF_RUST_LEDGER")
        .expect("temp Rust shadow ledger path required");
    let live = std::env::var("ASOLARIA_COSIGN_LIVE")
        .unwrap_or_else(|_| "C:/asolaria-acer/COSIGN_CHAIN.ndjson".to_string());

    assert_ne!(Path::new(&py_ledger), Path::new(&live));
    assert_ne!(Path::new(&rust_ledger), Path::new(&live));
    assert!(
        Path::new(&daemon).exists(),
        "patched daemon copy must exist"
    );
}
