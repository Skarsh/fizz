#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

DEFAULT_MODEL="qwen2.5:3b"
DEFAULT_OLLAMA_BASE_URL="http://localhost:11434"

log() {
  printf '[install] %s\n' "$*"
}

fail() {
  printf '[install] ERROR: %s\n' "$*" >&2
  exit 1
}

require_cmd() {
  local cmd="$1"
  command -v "$cmd" >/dev/null 2>&1 || fail "missing required command: ${cmd}"
}

read_env_var() {
  local file="$1"
  local key="$2"

  [[ -f "$file" ]] || return 1

  local line value
  line="$(grep -E "^[[:space:]]*${key}=" "$file" | tail -n 1 || true)"
  [[ -n "$line" ]] || return 1

  value="${line#*=}"
  value="$(printf '%s' "$value" | sed -e 's/[[:space:]]*#.*$//' -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
  value="${value%\"}"
  value="${value#\"}"
  value="${value%\'}"
  value="${value#\'}"

  [[ -n "$value" ]] || return 1
  printf '%s\n' "$value"
}

is_truthy() {
  local value="${1:-}"
  case "${value,,}" in
    1|true|yes|on) return 0 ;;
    *) return 1 ;;
  esac
}

wait_for_ollama() {
  local base_url="$1"
  local attempts=60
  local i

  for ((i = 1; i <= attempts; i++)); do
    if curl -fsS --max-time 2 "${base_url}/api/tags" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done

  return 1
}

start_ollama() {
  local ollama_gpu="${1:-0}"
  local compose_cmd=()

  if docker compose version >/dev/null 2>&1; then
    compose_cmd=(docker compose)
  elif command -v docker-compose >/dev/null 2>&1; then
    compose_cmd=(docker-compose)
  fi

  if [[ -f "${REPO_ROOT}/compose.yaml" && ${#compose_cmd[@]} -gt 0 ]]; then
    local compose_files=(-f "${REPO_ROOT}/compose.yaml")
    if is_truthy "$ollama_gpu"; then
      if [[ -f "${REPO_ROOT}/compose.gpu.yaml" ]]; then
        log "OLLAMA_GPU enabled; applying compose.gpu.yaml override."
        compose_files+=(-f "${REPO_ROOT}/compose.gpu.yaml")
      else
        log "OLLAMA_GPU enabled but compose.gpu.yaml is missing; continuing without GPU override."
      fi
    fi

    log "Starting Ollama with ${compose_cmd[*]}..."
    "${compose_cmd[@]}" "${compose_files[@]}" up -d ollama
    return
  fi

  if docker ps -a --format '{{.Names}}' | grep -Fxq 'ollama'; then
    if is_truthy "$ollama_gpu"; then
      log "OLLAMA_GPU enabled but existing ollama container settings are reused."
    fi
    log "Starting existing Ollama container..."
    docker start ollama >/dev/null
    return
  fi

  local docker_run_args=(
    -d
    --name ollama
    --restart unless-stopped
    -p 11434:11434
    -e OLLAMA_HOST=0.0.0.0:11434
    -v ollama-data:/root/.ollama
  )
  if is_truthy "$ollama_gpu"; then
    log "OLLAMA_GPU enabled; starting container with --gpus all."
    docker_run_args+=(--gpus all)
  fi

  log "Starting Ollama container with docker run..."
  docker run "${docker_run_args[@]}" ollama/ollama:latest >/dev/null
}

main() {
  cd "$REPO_ROOT"

  require_cmd cargo
  require_cmd docker
  require_cmd curl

  if ! docker info >/dev/null 2>&1; then
    fail "cannot access Docker daemon. Check Docker is running and your user has socket access."
  fi

  local model ollama_base_url model_provider ollama_gpu
  model_provider="$(read_env_var .env MODEL_PROVIDER || printf '%s' "ollama")"
  model="$(read_env_var .env MODEL || printf '%s' "$DEFAULT_MODEL")"
  ollama_base_url="$(read_env_var .env MODEL_BASE_URL || printf '%s' "$DEFAULT_OLLAMA_BASE_URL")"
  ollama_gpu="${OLLAMA_GPU:-$(read_env_var .env OLLAMA_GPU || printf '%s' "0")}"

  if [[ "${model_provider,,}" != "ollama" ]]; then
    log "MODEL_PROVIDER=${model_provider}; using local Ollama model ${DEFAULT_MODEL} for bootstrap."
    model="$DEFAULT_MODEL"
  fi

  start_ollama "$ollama_gpu"

  log "Waiting for Ollama API at ${ollama_base_url}..."
  if ! wait_for_ollama "$ollama_base_url"; then
    fail "Ollama did not become ready at ${ollama_base_url} within timeout."
  fi

  log "Pulling model ${model}..."
  docker exec ollama ollama pull "$model"

  log "Bootstrap complete."
}

main "$@"
