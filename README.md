# fizz

Basic Rust AI agent harness.

## Quick start

1. Start and bootstrap Ollama:

```bash
./scripts/install.sh
```

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

## Built-in tool

- `time.now`: returns current UTC time and unix time in seconds.
