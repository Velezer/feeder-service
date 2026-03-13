# Rust Tests Workflow Guardrails

`rust-tests.yml` is the required CI gate for push/pull_request on `main` and `dev`.

## What it enforces

1. Rust formatting stays deterministic (`cargo fmt --all -- --check`).
2. Unit + integration targets compile and execute (`cargo test --all-targets`).
3. Non-destructive e2e suites continue running in CI (news correlation/store/telegram rollout + notifier tests).

## Local preflight (recommended)

```bash
cargo fmt --all -- --check
cargo test --all-targets
cargo test --test news_correlation_e2e -- --nocapture
cargo test --test news_price_correlation_e2e -- --nocapture
cargo test --test news_store_symbol_index_e2e -- --nocapture
cargo test --test telegram_notifier_e2e -- --nocapture
cargo test --test news_telegram_rollout_env_e2e -- --nocapture
```

Running the same sequence locally avoids avoidable CI failures and keeps resource usage low by failing early on formatting.
## Current workflow set

- `.github/workflows/rust-tests.yml` is the only required repository CI workflow.
- The legacy AI-agent workflow has been removed to simplify CI behavior and maintenance.

