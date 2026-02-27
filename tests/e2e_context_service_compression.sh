#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d)"
LOG_FILE="$TMP_DIR/context-service.log"

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "$SERVER_PID" >/dev/null 2>&1 || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

git clone --depth 1 https://github.com/Velezer/context-service "$TMP_DIR/context-service" >/dev/null 2>&1

nohup cargo run --manifest-path "$TMP_DIR/context-service/Cargo.toml" >"$LOG_FILE" 2>&1 &
SERVER_PID=$!

for _ in {1..240}; do
  if curl -fsS "http://127.0.0.1:8080/context" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

if ! curl -fsS "http://127.0.0.1:8080/context" >/dev/null 2>&1; then
  echo "context-service failed to start"
  cat "$LOG_FILE"
  exit 1
fi

PAYLOAD=$(jq -n \
  --arg ctx "AI workflow context compression validation" \
  --arg wf "$(sed -n '1,220p' "$ROOT_DIR/.github/workflows/ai-agent.yml")" \
  --arg spec "$(sed -n '1,180p' "$ROOT_DIR/SPEC.md")" \
  '{
    context: $ctx,
    files: [
      {path: ".github/workflows/ai-agent.yml", content: $wf},
      {path: "SPEC.md", content: $spec}
    ],
    max_tokens: 300
  }')

RESPONSE=$(curl -fsS "http://127.0.0.1:8080/context/compress" \
  -H 'Content-Type: application/json' \
  -d "$PAYLOAD")

COMPRESSED=$(jq -r '.compressed_context' <<< "$RESPONSE")
TOKENS=$(jq -r '.estimated_tokens' <<< "$RESPONSE")

if [[ -z "$COMPRESSED" || "$COMPRESSED" == "null" ]]; then
  echo "Compression output missing"
  exit 1
fi

if ! [[ "$TOKENS" =~ ^[0-9]+$ ]]; then
  echo "Invalid token estimate: $TOKENS"
  exit 1
fi

echo "compressed_tokens=$TOKENS"
echo "$COMPRESSED" | head -n 10
