# fizz

Basic Rust AI agent harness.

## Quick start

1. Start and bootstrap Ollama:

```bash
./scripts/install.sh
```

Optional NVIDIA GPU bootstrap:

```bash
OLLAMA_GPU=1 ./scripts/install.sh
```

`OLLAMA_GPU` is a bootstrap flag consumed by `scripts/install.sh`; the app runtime ignores it.

2. Run one prompt:

```bash
cargo run -- "Write a haiku about Rust"
```

3. Run interactive mode:

```bash
cargo run
```

In REPL mode:
- `/history` prints the in-memory conversation transcript sent to the model.
- `/reset` clears conversation memory.

## Configuration

Environment variables (all optional):

- `MODEL_PROVIDER` (default: `ollama`)
- `MODEL` (default: `qwen2.5:3b`)
- `MODEL_BASE_URL` (default: `http://localhost:11434`)
- `SYSTEM_PROMPT` (default: `You are a helpful assistant.`)
- `MODEL_TIMEOUT_SECS` (default: `60`)

At startup, the app automatically loads values from a local `.env` file if present.

## Logging

Logging uses `tracing` and writes to stderr by default.

- Default filter: `warn,fizz=info`
- Default format: `pretty`
- Set `LOG_FORMAT=json` for machine-readable logs.
- Output routing via `LOG_OUTPUT=stderr|file|both` (default: `stderr`)
- File output path via `LOG_FILE_PATH` (default: `logs/fizz.log`, daily rotation)
- Turn, model, and tool logs are correlated by `turn_id`.
- Override with `RUST_LOG`, for example:

```bash
RUST_LOG=fizz=debug cargo run -- "Write a haiku about Rust"
```

```bash
LOG_FORMAT=json RUST_LOG=fizz=info cargo run -- "Write a haiku about Rust"
```

```bash
LOG_OUTPUT=file LOG_FILE_PATH=logs/fizz.log RUST_LOG=fizz=info cargo run -- "Write a haiku about Rust"
```

Recommended settings:
- Dev: `LOG_OUTPUT=stderr LOG_FORMAT=pretty RUST_LOG=fizz=debug`
- CI/Production-like runs: `LOG_OUTPUT=file LOG_FORMAT=json RUST_LOG=fizz=info`

## Built-in tool

- `time.now`: returns current UTC time and unix time in seconds.
