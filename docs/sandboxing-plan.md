# Sandboxed Tools Plan (Docker-Independent)

## Goal

Build a sandboxed tool runtime that is seamless for end users and does not require Docker (or any external container runtime) to be installed, while still allowing tools to use:

- Models exposed at local endpoints (including models running in Docker, e.g. `http://localhost:11434`).
- Online model providers (OpenAI/Anthropic/etc.).
- A virtual workspace filesystem (AgentFS-style copy-on-write sandboxing).

## Product Constraints

- Sandbox must run out-of-the-box with the app binary.
- Tool execution should be policy-driven (explicit allow/deny of capabilities).
- Model access from tools should be mediated by the host, not direct by default.
- Workspace access from tools should be virtualized by default (not direct host writes).
- Existing tool behavior should keep working during migration.

## Proposed Architecture

### 1) Runtime Layers

- `ToolRunner` trait: common interface for tool execution.
- `BuiltinRunner`: current in-process Rust tools (for compatibility).
- `WasmRunner` (new default target): executes tools in Wasmtime/WASI with capability controls.

### 2) Capability Mediation

- `ModelGateway` trait in host process:
  - Receives structured model requests from tools.
  - Uses existing provider path (`MODEL_PROVIDER`, `MODEL_BASE_URL`, timeouts, etc.).
  - Returns normalized model responses to tool.
- Default policy: deny arbitrary network from tools.
- Optional policy flag for direct network only when explicitly enabled.

### 3) Tool Contract

- Expand tool call schema from name-only to structured arguments:
  - `{"tool_call":{"name":"tool.name","arguments":{...}}}`
- Add tool manifest metadata (for sandbox policy + validation):
  - Name/version
  - Input schema
  - Capabilities (`model_access`, `network`, `filesystem`, `env`)
  - Resource limits (`timeout`, `memory`, `fuel/instruction budget`)

### 4) Sandboxing Model (Wasm)

- WASI host bindings with least privilege.
- Filesystem mounts are explicit and minimal, and target virtualized workspace paths.
- Environment variable allowlist only.
- Resource controls:
  - Per-tool timeout
  - Memory limit
  - Execution budget (fuel/epoch interruption)
- Clean error mapping for policy denials and runtime termination.

### 5) Virtual Workspace FS (AgentFS-Style)

- `WorkspaceFS` trait in host process with pluggable backends:
  - `host` backend for compatibility.
  - `overlay` backend (read-only base + writable delta) as a built-in sandbox backend.
  - `agentfs` backend (via AgentFS Rust SDK) for managed virtual filesystem sessions.
- Tool runtime sees a workspace mount (e.g. `/workspace`) that is backed by `WorkspaceFS`, not raw repo writes.
- Session operations supported by `WorkspaceFS`:
  - create session
  - read/write/list
  - diff against base
  - commit/discard
- Implementation path:
  - v1: in-process overlay backend (no external dependency required) plus optional `agentfs` backend when configured.
  - all backends must satisfy a shared `WorkspaceFS` contract test suite.

## Implementation Plan

## Phase 0: Baseline and Guardrails

### Scope

- Introduce feature flags and config scaffolding without changing runtime behavior.
- Preserve existing `time.now` path.

### Tasks

- Add config fields:
  - `TOOL_RUNTIME` (`builtin|wasm`, default `builtin` initially)
  - `TOOL_TIMEOUT_SECS`
  - `TOOL_MEMORY_MB`
  - `TOOL_ALLOW_DIRECT_NETWORK` (default `false`)
  - `WORKSPACE_FS_MODE` (`host|overlay|agentfs`, default `host` initially)
- Add internal policy structs and default policy.
- Document migration intent in `README`.

### Acceptance Criteria

- No behavior change for current users.
- Existing tests remain green.

## Phase 1: Execution Abstraction

### Scope

- Decouple agent loop from hardcoded `tools::execute`.

### Tasks

- Add `ToolRunner` trait and route calls through it in agent.
- Keep `BuiltinRunner` backed by current logic.
- Normalize tool execution result format and errors.

### Acceptance Criteria

- Agent behavior matches current behavior with `BuiltinRunner`.
- Unit tests for tool hop loop still pass.

## Phase 2: Model Gateway

### Scope

- Introduce host-side gateway for model access on behalf of tools.

### Tasks

- Add `ModelGateway` trait with request/response types.
- Implement gateway adapter over existing provider stack.
- Add policy check for `model_access` capability.
- Add `WorkspaceFS` trait and `host` backend implementation.
- Add `agentfs` backend adapter behind config/feature flag.

### Acceptance Criteria

- Tools can request model inference through host gateway.
- Works with:
  - Local URL model endpoints (including Docker-hosted models on localhost).
  - Online providers via provider config.

## Phase 3: Wasm Runner MVP

### Scope

- Add Wasmtime-based runner for simple tools.

### Tasks

- Add `wasmtime` + WASI integration.
- Define stdin/stdout JSON protocol for tool invocation.
- Implement policy enforcement:
  - Default no network
  - Explicit FS mounts only (workspace mount supplied by `WorkspaceFS`)
  - Env allowlist
  - Timeout + memory + fuel budget
- Add `overlay` `WorkspaceFS` backend with session lifecycle (create/diff/commit/discard).
- Implement backend-parity checks so `overlay` and `agentfs` expose equivalent session semantics.
- Add one reference wasm tool fixture for integration testing.

### Acceptance Criteria

- A wasm tool can execute with constrained capabilities.
- Policy-denied actions fail with actionable errors.

## Phase 4: Developer Workflow

### Scope

- Make it easy to build/test tools locally without external runtime requirements.

### Tasks

- Add `tools/` workspace layout:
  - `tools/<tool-name>/manifest.json`
  - source + build scripts
- Add `cargo` helper commands or scripts:
  - build tool
  - run tool in sandbox
  - start workspace session / diff / commit / discard
  - run integration tests
- Add examples for:
  - no-capability tool
  - model-access tool
  - workspace-read/write tool under overlay and `agentfs` modes

### Acceptance Criteria

- Developer can create and run a sandboxed tool with one documented command sequence.
- Docs clearly distinguish host model config vs tool capabilities.

## Phase 5: Schema and Migration

### Scope

- Move from name-only tool calls to argument-aware calls and validation.

### Tasks

- Extend parsing to support `arguments`.
- Add per-tool input validation.
- Keep backward compatibility for legacy name-only calls during transition.

### Acceptance Criteria

- Invalid tool arguments are rejected with clear messages.
- Legacy calls continue to work for existing prompts.

## Phase 6: CI and Hardening

### Scope

- Add automated safety/regression coverage for sandbox behavior.

### Tasks

- Unit tests:
  - policy parsing/defaults
  - capability checks
  - error mapping
- Integration tests:
  - network denied by default
  - filesystem writes blocked unless mounted
  - overlay writes are isolated from host workspace until commit
  - discard removes overlay changes
  - diff output matches edited files
  - `agentfs` backend passes the same workspace contract tests (when enabled)
  - timeout and execution budget enforcement
  - model gateway path works end-to-end
- CI jobs:
  - fast unit lane
  - sandbox integration lane

### Acceptance Criteria

- CI fails on sandbox escape regressions.
- Deterministic tests without Docker dependency.

## Test Strategy

### Core Test Matrix

- Runtime: `builtin`, `wasm`.
- Workspace FS mode: `host`, `overlay`, `agentfs`.
- Model path:
  - localhost endpoint configured in host
  - mock online provider endpoint
- Capability sets:
  - none
  - model-only
  - model+fs
  - direct network (explicit opt-in)

### Failure-Mode Tests

- Tool exceeds timeout.
- Tool exceeds memory/fuel budget.
- Tool requests unauthorized capability.
- Tool attempts host-path write outside virtual workspace.
- Gateway/provider errors bubble up with actionable messages.

## Security Defaults

- Deny-by-default capability policy.
- No direct tool network unless explicitly enabled.
- Model access only via host broker by default.
- Tools write to virtual workspace mounts by default; host writes require explicit mode/policy.
- Explicit, minimal filesystem mounts.
- Explicit environment variable allowlist.

## Open Design Decisions

- Wasm component model vs plain WASI ABI for v1 protocol.
- Exact memory/fuel defaults (developer-friendly vs strict).
- Whether to support native-tool execution mode at all (likely optional, not default).
- Whether default workspace mode should remain `overlay` or move to `agentfs` when available.
- Manifest format ownership and versioning strategy.

## Rollout Strategy

- Step 1: ship abstraction and policy plumbing behind defaults that preserve behavior.
- Step 2: ship `WorkspaceFS` (`host` mode) + wasm runner as opt-in (`TOOL_RUNTIME=wasm`).
- Step 3: ship `overlay` and `agentfs` workspace modes as opt-in (`WORKSPACE_FS_MODE=overlay|agentfs`).
- Step 4: migrate built-in/new tools to wasm where practical.
- Step 5: consider switching defaults to `TOOL_RUNTIME=wasm` and a virtual workspace mode (`overlay` or `agentfs`) after stability metrics are met.

## Definition of Done (Project-Level)

- Users can run sandboxed tools without Docker installed.
- Tools can perform model calls through host gateway across local and online providers.
- Tools can operate in a virtual copy-on-write workspace with diff/commit/discard flow via `overlay` or `agentfs`.
- Default policy prevents direct network/file access unless explicitly permitted.
- CI includes deterministic sandbox enforcement tests.
- Existing non-sandbox workflows remain functional during migration.

## Implementation Tracker (Checklist)

Use this section to track progress during implementation. Check items as they are completed.

### Status Legend

- `[ ]` not started
- `[~]` in progress (mark manually)
- `[x]` complete
- `[!]` blocked (mark manually with short reason)

### Phase Completion Board

- [ ] Phase 0 complete
- [ ] Phase 1 complete
- [ ] Phase 2 complete
- [ ] Phase 3 complete
- [ ] Phase 4 complete
- [ ] Phase 5 complete
- [ ] Phase 6 complete

## Phase 0 Detailed Checklist (Baseline and Guardrails)

### Deliverables

- [ ] `P0.1` Add `TOOL_RUNTIME` config parsing (`builtin|wasm`).
- [ ] `P0.2` Add `TOOL_TIMEOUT_SECS` config parsing + defaults + tests.
- [ ] `P0.3` Add `TOOL_MEMORY_MB` config parsing + defaults + tests.
- [ ] `P0.4` Add `TOOL_ALLOW_DIRECT_NETWORK` config parsing + defaults + tests.
- [ ] `P0.5` Add `WORKSPACE_FS_MODE` config parsing (`host|overlay|agentfs`) + tests.
- [ ] `P0.6` Add initial internal policy structs with default values.
- [ ] `P0.7` Update `README` with new env vars and note that behavior is unchanged.

### Suggested Touchpoints

- [ ] `src/config.rs`
- [ ] `src/lib.rs` (if config wiring requires it)
- [ ] `.env.example`
- [ ] `README.md`

### Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-targets --all-features`

## Phase 1 Detailed Checklist (Execution Abstraction)

### Deliverables

- [ ] `P1.1` Introduce `ToolRunner` trait.
- [ ] `P1.2` Add `BuiltinRunner` implementing `ToolRunner` using current behavior.
- [ ] `P1.3` Route agent turn loop through `ToolRunner` (remove direct dependency on `tools::execute` in turn engine path).
- [ ] `P1.4` Keep existing `time.now` behavior identical.
- [ ] `P1.5` Normalize tool execution error/result type for later wasm and workspace backends.

### Suggested Touchpoints

- [ ] `src/agent/mod.rs`
- [ ] `src/agent/tools.rs` (or split into `runner` + builtin impl modules)
- [ ] new module(s) for runner traits/types (location chosen during implementation)

### Verification

- [ ] Existing unit tests for tool hop loop remain unchanged and passing.
- [ ] Add/adjust unit tests proving `ToolRunner` abstraction does not alter outputs.
- [ ] `cargo test --all-targets --all-features`

## Phase 2 Detailed Checklist (Model Gateway + WorkspaceFS Abstractions)

### Deliverables

- [ ] `P2.1` Introduce `ModelGateway` trait and typed request/response structs.
- [ ] `P2.2` Implement host-backed gateway adapter using current provider stack.
- [ ] `P2.3` Enforce policy check for `model_access` capability.
- [ ] `P2.4` Introduce `WorkspaceFS` trait with session lifecycle methods.
- [ ] `P2.5` Implement `host` `WorkspaceFS` backend.
- [ ] `P2.6` Add `agentfs` backend adapter behind feature/config flag.

### Suggested Touchpoints

- [ ] `src/model.rs` and/or new gateway module(s)
- [ ] `src/providers/*` (reuse existing provider code through gateway)
- [ ] new workspace fs module(s), for example `src/workspace_fs/*`
- [ ] config wiring for backend selection

### Verification

- [ ] Unit tests for `ModelGateway` request/response mapping.
- [ ] Unit tests for policy deny path when `model_access` is not granted.
- [ ] Contract tests for `WorkspaceFS` trait against `host` backend.

## Phase 3 Detailed Checklist (Wasm Runner MVP + Overlay Backend)

### Deliverables

- [ ] `P3.1` Add Wasmtime/WASI dependencies and runtime bootstrap.
- [ ] `P3.2` Implement wasm tool invocation protocol (stdin JSON -> stdout JSON).
- [ ] `P3.3` Enforce timeout + memory + fuel/instruction budget.
- [ ] `P3.4` Enforce default network deny for wasm tools.
- [ ] `P3.5` Enforce explicit mount policy; mount workspace via `WorkspaceFS`.
- [ ] `P3.6` Implement built-in `overlay` backend with create/diff/commit/discard.
- [ ] `P3.7` Add contract test suite ensuring `overlay` and `agentfs` semantics match where applicable.

### Suggested Touchpoints

- [ ] new wasm runtime module(s), for example `src/tools/wasm/*`
- [ ] new overlay workspace module(s), for example `src/workspace_fs/overlay/*`
- [ ] tool policy module(s)
- [ ] agent wiring to select runtime/backend by config

### Verification

- [ ] Integration test: tool network access denied by default.
- [ ] Integration test: tool cannot write outside allowed workspace mount.
- [ ] Integration test: timeout/fuel/memory limit failures are surfaced clearly.
- [ ] Integration test: overlay `diff` and `discard` behavior.

## Phase 4 Detailed Checklist (Developer Workflow)

### Deliverables

- [ ] `P4.1` Create `tools/<tool-name>/manifest.json` structure and schema doc.
- [ ] `P4.2` Add scripts or cargo aliases for:
- [ ] `P4.2.a` build tool
- [ ] `P4.2.b` run in sandbox
- [ ] `P4.2.c` workspace session create/diff/commit/discard
- [ ] `P4.2.d` run integration tests
- [ ] `P4.3` Provide example tools:
- [ ] `P4.3.a` no-capability tool
- [ ] `P4.3.b` model-access tool
- [ ] `P4.3.c` workspace read/write tool (overlay + agentfs modes)
- [ ] `P4.4` Add developer documentation for choosing `WORKSPACE_FS_MODE`.

### Suggested Touchpoints

- [ ] `tools/` directory
- [ ] `scripts/` helper scripts and/or cargo aliases
- [ ] `README.md`
- [ ] docs under `docs/` for tool authoring workflow

### Verification

- [ ] New contributor can run example tool end-to-end using documented commands.
- [ ] Tool authoring docs include expected input/output protocol and capability declaration.

## Phase 5 Detailed Checklist (Schema and Migration)

### Deliverables

- [ ] `P5.1` Extend tool call parser to accept `arguments`.
- [ ] `P5.2` Introduce per-tool input validation path.
- [ ] `P5.3` Preserve backward compatibility for name-only tool calls.
- [ ] `P5.4` Update system tool usage instructions to include argument schema.
- [ ] `P5.5` Add explicit error messages for malformed/invalid arguments.

### Suggested Touchpoints

- [ ] `src/agent/tools.rs` parser + instructions
- [ ] tool execution path where validated arguments are passed
- [ ] tests for parser/validation/backward compatibility

### Verification

- [ ] Unit tests for valid/invalid argument payloads.
- [ ] Regression tests proving legacy calls still execute.

## Phase 6 Detailed Checklist (CI and Hardening)

### Deliverables

- [ ] `P6.1` Add shared `WorkspaceFS` contract tests runnable against `host`, `overlay`, and `agentfs` (when enabled).
- [ ] `P6.2` Add sandbox integration tests for security boundaries.
- [ ] `P6.3` Add integration tests for model gateway behavior (localhost and mocked online providers).
- [ ] `P6.4` Split CI lanes into fast unit and sandbox integration jobs.
- [ ] `P6.5` Ensure CI configuration handles optional `agentfs` backend mode cleanly.

### Suggested Touchpoints

- [ ] test modules under `src/*` and/or `tests/`
- [ ] `.github/workflows/ci.yml`
- [ ] docs describing required CI environment assumptions

### Verification

- [ ] CI fails on policy regressions (network/file/capability boundaries).
- [ ] CI remains deterministic without requiring Docker.

## Optional Backlog (Post-v1)

- [ ] `B1` Explore defaulting `WORKSPACE_FS_MODE=agentfs` when environment support is detected.
- [ ] `B2` Add native tool execution mode (non-default) with stricter guardrails.
- [ ] `B3` Add richer telemetry for sandbox policy denials and resource-limit terminations.
- [ ] `B4` Add prompt/tooling guidance templates for argument-rich tool calls.
