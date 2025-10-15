# wasm-rr Repository Reference

A deterministic record-replay tool for WebAssembly components, capturing and replaying non-deterministic host calls including clocks, random values, environment variables, and HTTP responses.

## Structure

```
src/main.rs         # CLI entrypoint
src/recorder.rs     # Record logic
src/playback.rs     # Replay logic
examples/*/         # Sample WASM components
target/             # Build artifacts
result/*.wasm       # Nix build output
rust-toolchain.toml # Wasmtime toolchain pin
flake.nix           # Nix dev shell
```

## Commands

```bash
cargo build [--release]                                    # Build CLI
cargo run -- record <wasm> [-t <trace>] [-- <args>]       # Record execution (default: wasm-rr-trace.json)
cargo run -- replay <wasm> [<trace>]                       # Replay trace (default: wasm-rr-trace.json)
nix build .                                                # Build examples + CLI
cargo test [-- --nocapture]                               # Run tests
cargo fmt && cargo clippy --all-targets --all-features    # Lint
```

## Code Standards

- Rust 2021, 4-space indent
- `snake_case` modules, `CamelCase` types, `SCREAMING_SNAKE_CASE` constants
- Error propagation: `anyhow::Context`
- Pre-commit: `cargo fmt`, address `clippy` warnings
- Preserve serde tags in `TraceEvent` variants

## Testing

- Unit tests: Colocate with modules
- Integration tests: `tests/` directory
- Golden tests: See § Golden Testing
- Bug fixes: Add example in `examples/`, record trace
- Validation: `record` → `replay` parity check

## Commits

Conventional Commits format:

```
<type>[(<scope>)]: <description>
```

Types: `feat`, `fix`, `refactor`, `chore`, `docs`, `test`

Keep commit descriptions on points and refrain from adding "Co-authored by ..."

PR requirements:

- Link issue
- Validation steps + outputs
- New traces/screenshots
- Note breaking changes

## Feature Plans

Required sections:

```markdown
# <Feature> Plan

- Summary: <one-sentence>
- Owner: <name>
- Last updated: <date>

## Description

<scope, motivation, constraints>

## Success Criteria

- [ ] <measurable outcome>

## Resources

- <specs, docs, prior art>

## Risks & Mitigations

- Risk: <issue> → Mitigation: <approach>

## Implementation

- [ ] <task with validation command>

## Open Questions

- <unresolved items>

## Decision Log

- YYYY-MM-DD — <decision> — <rationale>
```

## Golden Testing

### Structure

```
golden/<component>/
├── metadata.toml  # Component ref + flags
├── trace.json     # Execution trace
├── stdout.txt     # Expected stdout
└── stderr.txt     # Expected stderr
```

### Commands

```bash
nix run .#golden-test     # Run all golden tests
nix run .#golden-fixture  # Record new fixture
```

### metadata.toml

```toml
component = "<name>"
trace = "trace.json"
stdout = "stdout.txt"
stderr = "stderr.txt"
must_fail = true  # Optional: mark expected failures
```

### Adding Tests

1. Create `examples/<name>/`
2. Build: `nix build .`
3. Record: `nix run .#golden-fixture -- <name>`
4. Update `flake.nix` examples list
5. Update `resolve_wasm()` in `nix/golden-test.sh`

### must_fail Behavior

- `must_fail = true` + test fails → ✓ Expected failure
- `must_fail = true` + test passes → ✗ Unexpected pass (fails suite)

## Implementation Patterns

### WASI Component Integration

Pattern for adding WASI interfaces to linker:

```rust
use wasmtime_wasi::p2::bindings::<interface_module>::<interface>;
use wasmtime_wasi::<InterfaceView>;

<interface>::add_to_linker::<T, ViewType>(&mut linker, |t| t.view_method())?;
```

Example (wasi:random/random):

```rust
use wasmtime_wasi::p2::bindings::random::random;
use wasmtime_wasi::{WasiRandom, WasiRandomView};

random::add_to_linker::<T, WasiRandom>(&mut linker, |t| t.random())?;
```

### Recording/Replay Pattern

For deterministic replay of non-deterministic host calls:

1. Extend `TraceEvent` enum:

```rust
enum TraceEvent {
    // ...
    RandomBytes { bytes: Vec<u8> },
    RandomU64 { value: u64 },
}
```

2. Implement recording (`CtxRecorder`):

```rust
impl random::random::Host for CtxRecorder {
    fn get_random_bytes(&mut self, len: u64) -> Result<Vec<u8>> {
        let bytes = self.table.random().get_random_bytes(len)?;
        self.events.push(TraceEvent::RandomBytes { bytes: bytes.clone() });
        Ok(bytes)
    }
}
```

3. Implement replay (`CtxPlayback`):

```rust
impl random::random::Host for CtxPlayback {
    fn get_random_bytes(&mut self, len: u64) -> Result<Vec<u8>> {
        match self.events.next() {
            Some(TraceEvent::RandomBytes { bytes }) => Ok(bytes),
            _ => bail!("Trace mismatch"),
        }
    }
}
```

4. Use `Intercept<T>` in linker:

```rust
random::random::add_to_linker::<_, Intercept<T>>(&mut linker, |t| &mut t.inner)?;
```

## PR Workflow for Agents

When creating a pull request, follow this workflow:

1. **Fetch latest changes from main**
   ```bash
   git fetch origin main
   ```

2. **Create a new branch based on updated main**
   ```bash
   git checkout -b <descriptive-branch-name> origin/main
   ```

3. **Make your changes**
   - Follow code standards (see § Code Standards)
   - Run tests and linting before committing
   - Write clear commit messages using Conventional Commits format

4. **Create a draft PR early**
   ```bash
   gh pr create --draft --title "<type>: <description>" --body "Work in progress"
   ```

   Creating a draft PR early allows:
   - Early visibility of work in progress
   - CI checks to run on pushes
   - Easier collaboration and feedback

5. **Continue development**
   - Push commits as you work
   - CI will run on each push to the PR branch

6. **Mark PR as ready when complete**
   ```bash
   gh pr ready <number>
   ```

## GitHub CLI Cheatsheet

```bash
# Pull Request Management
gh pr create --title "title" --body "description"          # Create PR
gh pr create --draft                                        # Create draft PR
gh pr list                                                   # List PRs
gh pr view <number>                                          # View PR details
gh pr checkout <number>                                      # Checkout PR branch
gh pr review <number> --approve                             # Approve PR
gh pr merge <number>                                         # Merge PR
gh pr close <number>                                         # Close PR

# Issue Management
gh issue create --title "title" --body "description"        # Create issue
gh issue list                                               # List issues
gh issue view <number>                                      # View issue
gh issue close <number>                                      # Close issue

# Repository Operations
gh repo clone <owner>/<repo>                                # Clone repository
gh repo fork                                                # Fork repository
gh repo view                                                # View repo info

# Workflow/CI
gh run list                                                  # List workflow runs
gh run view <id>                                            # View run details
gh run watch <id>                                           # Watch run progress
gh workflow list                                            # List workflows
```

## Nix Cheatsheet

```bash
# Building
nix build .                                                  # Build default package
nix build .#<package>                                        # Build specific package
nix build .#wasm-rr                                         # Build CLI tool
nix build .#print_time-wasm                                 # Build example component

# Running
nix run .                                                    # Run default package
nix run .#golden-test                                        # Run all golden tests
nix run .#golden-fixture -- <name>                          # Record golden fixture

# Development
nix develop                                                  # Enter dev shell
nix develop -c <command>                                     # Run command in dev shell
nix flake check                                              # Run all checks
nix flake show                                               # Show flake outputs
nix flake update                                             # Update flake inputs

# Evaluation and Debugging
nix eval .#<attribute>                                       # Evaluate expression
nix repl                                                     # Interactive Nix REPL
nix log <store-path>                                         # View build logs
nix why-depends <package> <dependency>                      # Show dependency chain

# Common Patterns
nix build . && ./result/bin/wasm-rr                         # Build and run
nix develop -c cargo test                                   # Run tests in dev shell
nix flake check --print-build-logs                          # Check with logs
```

## Experience Log

### 2025-10-13: wasi:random/random

- Module path: `wasmtime_wasi::p2::bindings::random::random`
- View trait: `WasiRandom`, `WasiRandomView`
- Pattern: `module::interface::add_to_linker::<T, ViewType>(&mut linker, |t| t.view_method())`

### 2025-10-13: Random Value Recording

- Extended `TraceEvent` with `RandomBytes`, `RandomU64`
- Implemented `random::random::Host` for `CtxRecorder`/`CtxPlayback`
- Used `Intercept<T>` pattern for host call forwarding
- Result: Deterministic replay of random values (fixed `print_random` golden test)

### 2025-10-15: Extended Interface Support

- Added recording/replay for: environment variables, arguments, initial CWD
- Implemented wasi:http support with request/response recording
- TraceEvent variants now include: `ClockNow`, `ClockResolution`, `Environment`, `Arguments`, `InitialCwd`, `RandomBytes`, `RandomU64`, `HttpResponse`
