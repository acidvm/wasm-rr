# Repository Guidelines

## Project Structure & Module Organization

The CLI entrypoint lives in `src/main.rs`, with record/replay logic in
`src/recorder.rs` and `src/playback.rs`. Sample WebAssembly components reside in
`examples/`, each with its own Cargo manifest; build them via `nix build .`,
which writes artifacts to `result/*.wasm`. Generated artifacts for the CLI
collect in `target/`; `rust-toolchain.toml` pins the toolchain expected by
Wasmtime, and `flake.nix` offers a reproducible dev shell for Nix users.

## Build, Test, and Development Commands

- `cargo build` – compiles the CLI; add `--release` when benchmarking replay
  fidelity.
- `cargo run -- record result/print_time.wasm -t traces/time.json -- --flag` –
  records clock calls while forwarding trailing args to the component.
- `cargo run -- replay …` – replays a previously captured trace; ensure the
  `.wasm` binary matches the trace origin.
- `nix build .` – compiles the example Wasm components and the CLI dependencies
  into `result/`.
- `cargo test` – runs unit and doc tests; use `cargo test -- --nocapture` when
  you need stdout.

## Coding Style & Naming Conventions

Use Rust 2021 idioms with 4-space indentation; keep modules `snake_case`, types
`CamelCase`, and constants `SCREAMING_SNAKE_CASE`. Run `cargo fmt` before
submitting and prefer idiomatic error propagation via `anyhow::Context`. `cargo
clippy --all-targets --all-features` is encouraged; heed lint suggestions unless
there is a compelling reason not to. Trace event variants should remain
descriptive but concise while preserving existing serde tags.

## Testing Guidelines

Favor unit tests alongside the modules they cover; add integration tests under
`tests/` when exercising the full record/replay loop. Keep traces committed for
deterministic tests small and scrubbed of environment secrets. When fixing bugs,
reproduce them with a focused Wasm example under `examples/` and record the
expected trace. Manual validation should include a fresh `cargo run -- record`
followed by `cargo run -- replay` to confirm parity.

## Commit & Pull Request Guidelines

Commits must follow Conventional Commits
(https://www.conventionalcommits.org/en/v1.0.0/), e.g., `feat: add deterministic
playback cache` or `fix(playback): guard empty trace`. Use semantic types
(`feat`, `fix`, `refactor`, `chore`, `docs`, `test`) and scope when it adds
clarity. Keep bodies concise and explain the rationale, not the diff. Group
related changes so each commit compiles and passes tests. PRs must cite the
reproduced issue, summarize validation steps (commands plus outcomes), and
attach any new traces or screenshots that illustrate behavior changes. Link
issues where relevant and note any backward-compatibility concerns or follow-up
work in the description.

## Feature Plans

Feature plans are living markdown documents that evolve as you work through a
proposed feature or change. Each plan must include:

- A single-sentence summary highlighting the feature intent.
- A description that explains scope, motivation, and relevant constraints.
- A resource list linking to specs, docs, prior art, or other references.
- A checklist that decomposes the work and tracks progress, with owners and
  target dates when known.

To make plans actionable for agent-assisted development, also capture:

- Success criteria that clarify how we will evaluate the change (metrics,
  qualitative outcomes, or regression thresholds).
- Dependencies and integration points, including other agents, services, or
  external approvals.
- Risks and mitigations, especially around safety, privacy, or drift in
  autonomous behavior.
- Open questions and decision log entries so collaborators and agents can
  resolve them incrementally.

When in doubt, describe validation paths explicitly (e.g., which `cargo` or
`nix run` command exercises the change) and call out dependencies that should be
packaged as tooling or scripts rather than ad-hoc shell steps. Prefer reusable
automations for integration-style checks that rely on prebuilt artifacts or
shared environments, and surface them directly in the plan so agents can invoke
them. Embed telemetry or evaluation plans where applicable; agents can help run
these checks, but they need the expectations documented up front. Keep the plan
current as decisions shift, revising sections instead of letting them drift out
of date. Consider starting from a template like:

```markdown
# <Feature Name> Plan

- Summary: …
- Owner: …
- Last updated: …

## Description

…

## Success Criteria

- [ ] …

## Resources

- …

## Risks & Mitigations

- Risk: …  
  Mitigation: …

## Implementation steps

- [ ] Task

## Open Questions

- …

## Decision Log

- YYYY-MM-DD — Decision — Rationale
```

## Golden Testing

Golden testing in wasm-rr provides automated verification that replay outputs match expected results. The test infrastructure is implemented via Nix and consists of:

### Golden Test Structure

Golden fixtures are stored under `golden/<component>/` with the following files:
- `metadata.toml` - Contains component name and references to other files
- `trace.json` - The recorded execution trace
- `stdout.txt` - Expected stdout output
- `stderr.txt` - Expected stderr output

### Running Golden Tests

- `nix run .#golden-test` - Runs all golden tests, replaying traces and comparing outputs
- `nix run .#golden-fixture` - Helper to record new golden fixtures from example components

### Golden Test Implementation

The golden test runner (`nix/golden-test.sh`) iterates through all `metadata.toml` files, replays the associated trace with the corresponding WASM component, and compares actual vs expected stdout/stderr using unified diff. Tests pass when replay outputs exactly match the golden fixtures.

Currently supported examples with golden tests:
- `print_args` - Tests argument passing and environment variables
- `print_time` - Tests deterministic time functions

### Adding New Golden Tests

1. Create the example component under `examples/<name>/`
2. Build it with `nix build .`
3. Record golden fixtures with `nix run .#golden-fixture -- <component>`
4. Add the component to the `examples` list in `flake.nix`
5. Update `resolve_wasm()` in `nix/golden-test.sh` to include the new component

### Known Failures Support

Golden tests can be marked as expected failures using the `must_fail` flag in `metadata.toml`. This is useful for components with non-deterministic behavior that cannot yet be properly replayed:

```toml
component = "print_random"
trace = "trace.json"
stdout = "stdout.txt"
stderr = "stderr.txt"
must_fail = true  # Mark as expected failure
```

When a test is marked with `must_fail = true`:
- If the test fails (replay doesn't match expected output): Shows "✓ Expected failure"
- If the test passes unexpectedly: Shows "✗ Unexpected pass" and fails the test suite

This allows tracking components with known issues (like `print_random` which uses random values not yet captured in traces) without breaking CI. When the underlying issue is fixed, simply remove or set `must_fail = false` to convert it to a regular test.

## Experience Reports

### Adding wasi:random/random Component (2025-10-13)

The key insight for adding missing WASI components to the linker was understanding the module structure in `wasmtime_wasi::p2::bindings`. Each WASI interface (e.g., `wasi:random/random`) maps to a nested module path (`random::random`) with an `add_to_linker` function. The pattern follows: import the module from bindings, import the corresponding view trait (e.g., `WasiRandom` and `WasiRandomView`), then call `module::interface::add_to_linker::<T, ViewType>(&mut linker, |t| t.view_method())`. This consistent pattern across all WASI components made it straightforward to add the missing random component once the [wasmtime-wasi docs](https://docs.rs/wasmtime-wasi/37.0.1/wasmtime_wasi/random/struct.WasiRandom.html) clarified the exact types and module paths needed.

### Implementing Random Value Recording/Replay (2025-10-13)

Fixed the failing `print_random` golden test by implementing full recording and replay of random values. The solution followed the same pattern as clock interception:

1. **Added trace events** - Extended `TraceEvent` enum with `RandomBytes` and `RandomU64` variants to capture random values
2. **Implemented recording** - Added `random::random::Host` trait implementation for `CtxRecorder` that calls the real WASI random implementation and records the returned values
3. **Implemented replay** - Added `random::random::Host` trait implementation for `CtxPlayback` that returns previously recorded random values from the trace
4. **Updated linker** - Changed from using `WasiRandom` directly to using `Intercept<T>` for the random component, enabling our custom recording/replay logic

The key insight was that the `Intercept<T>` pattern allows forwarding host calls to the appropriate context (CtxRecorder during recording, CtxPlayback during replay), similar to how clocks and environment variables are handled. After these changes, `print_random` produces deterministic output during replay, converting it from a known failure (`must_fail = true`) to a passing test.
