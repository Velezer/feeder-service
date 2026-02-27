# AI Agent Workflow with `context-service` Compression

This repository's AI automation workflow now integrates with [`Velezer/context-service`](https://github.com/Velezer/context-service) to control prompt size and keep repository context under a token budget.

## Why this change

The previous workflow assembled raw file excerpts and then asked an LLM to summarize each file. That approach still generated oversized prompts and required extra model calls. The new flow performs deterministic token-budgeted compression through `context-service` before the final coding prompt.

## Current workflow flow

1. Trigger from issue creation, PR comment, or changes-requested review.
2. Score candidate files by keyword overlap with the issue/review text.
3. Select top files and send `{context, files, max_tokens}` to `POST /context/compress`.
4. Use `compressed_context` as the sole repository summary in the final LLM prompt.
5. Apply model output, commit, push, and optionally open PR.

## Local usage

### 1) Run the end-to-end compression test

```bash
cargo test --test context_service_compression_e2e -- --nocapture
```

This test:

- clones `Velezer/context-service`
- runs it locally on `127.0.0.1:8080`
- submits real repository files to `/context/compress`
- validates non-empty compressed output and numeric token estimate

### 2) Manual compression check

```bash
git clone --depth 1 https://github.com/Velezer/context-service /tmp/context-service
cargo run --manifest-path /tmp/context-service/Cargo.toml
```

In another terminal:

```bash
curl -sS http://127.0.0.1:8080/context/compress \
  -H 'Content-Type: application/json' \
  -d '{
    "context": "AI workflow task",
    "files": [{"path": "SPEC.md", "content": "sample"}],
    "max_tokens": 200
  }' | jq
```

## Best-practice notes

- Keep `max_tokens` bounded (current workflow uses `1800`) to avoid large-context failures.
- Avoid adding binary or generated files to the compression payload.
- Keep selected file count small (workflow currently uses top 10 scored files) for fast deterministic compression.
