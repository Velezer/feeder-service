use std::fs;

#[test]
fn rust_tests_workflow_keeps_format_gate_and_core_test_steps() {
    let workflow = fs::read_to_string(".github/workflows/rust-tests.yml")
        .expect("rust-tests workflow should be readable");

    assert!(
        workflow.contains("- name: Auto-format code"),
        "workflow should keep auto-format step on push"
    );
    assert!(
        workflow.contains("cargo fmt --all -- --check"),
        "workflow should enforce format check"
    );
    assert!(
        workflow.contains("cargo test --all-targets"),
        "workflow should run all-target tests"
    );
    assert!(
        workflow.contains("cargo test --test telegram_notifier_e2e -- --nocapture"),
        "workflow should keep non-destructive e2e coverage"
    );
}

#[test]
fn ai_agent_workflow_is_not_present_anymore() {
    assert!(
        fs::metadata(".github/workflows/ai-agent.yml").is_err(),
        "ai-agent workflow should be removed"
    );
}
