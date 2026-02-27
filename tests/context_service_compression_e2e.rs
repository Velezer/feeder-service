use std::process::Command;

#[test]
fn context_service_compression_flow_works_end_to_end() {
    let status = Command::new("bash")
        .arg("tests/e2e_context_service_compression.sh")
        .status()
        .expect("failed to execute e2e compression script");

    assert!(status.success(), "e2e compression script failed");
}
