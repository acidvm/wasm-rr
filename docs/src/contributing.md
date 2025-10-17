# Contributing

We welcome contributions to `wasm-rr`! This page explains how to get started.

## Development Setup

### Prerequisites

- [Rust](https://rustup.rs) via rustup
- [Nix](https://docs.determinate.systems) with flakes enabled

### Clone and Build

```bash
git clone https://github.com/acidvm/wasm-rr.git
cd wasm-rr

# Enter development shell
nix develop

# Build the project
cargo build

# Run tests
cargo test

# Build WASM examples
nix build .#wasm-examples
```

## Code Style

We follow standard Rust conventions with strict clippy lints:

### Naming

- Modules: `snake_case`
- Types: `CamelCase`
- Functions: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`

### Error Handling

Use `anyhow::Result` with context:

```rust
use anyhow::{Context, Result};

fn load_trace(path: &Path) -> Result<Trace> {
    File::open(path)
        .with_context(|| format!("failed to open trace at {}", path.display()))?;
    // ...
}
```

### Formatting and Linting

Before committing:

```bash
cargo fmt
cargo clippy --all-targets --all-features
```

All clippy warnings must be addressed.

### Strict Lints

The project enforces strict clippy lints:

```rust
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
#![forbid(unsafe_code)]
```

This means:

- No `.unwrap()` - use `?` or explicit error handling
- No `.expect()` - use proper `Result` types
- No `panic!()` - return errors instead
- No `unreachable!()` - use exhaustive matching
- No `todo!()` or `unimplemented!()` - finish code before committing
- No unsafe code

## Testing

### Unit Tests

Write inline tests in the same file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        // ...
    }
}
```

Run with:

```bash
cargo test
```

### Golden Tests

Golden tests verify end-to-end behavior. They're located in `golden/*/`:

```
golden/example/
├── metadata.toml    # Test config
├── trace.json       # Expected trace
├── stdout.txt       # Expected stdout
└── stderr.txt       # Expected stderr
```

Run all golden tests:

```bash
nix run .#golden-test
```

Record a new golden fixture:

```bash
nix run .#golden-fixture -- <component-name>
```

### Property-Based Tests

We use `quickcheck` for property-based testing:

```rust
#[cfg(test)]
mod tests {
    use quickcheck_macros::quickcheck;

    #[quickcheck]
    fn roundtrip_property(input: TestData) -> Result<bool> {
        // Test property holds for all inputs
        Ok(transform(input) == input)
    }
}
```

## Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>[(<scope>)]: <description>

[optional body]
```

Types:

- `feat`: New feature
- `fix`: Bug fix
- `refactor`: Code refactoring
- `chore`: Maintenance tasks
- `docs`: Documentation updates
- `test`: Test additions or changes

Examples:

```
feat: add support for wasi:filesystem recording
fix(replay): handle EOF errors correctly
refactor: extract CBOR utilities to separate module
docs: add architecture documentation
test: add golden test for HTTP recording
```

Keep descriptions concise and avoid unnecessary attribution (no "Co-authored-by" unless truly pair programming).

## Pull Request Process

### 1. Create a Branch

```bash
git fetch origin main
git checkout -b feature-name origin/main
```

### 2. Make Changes

- Follow code standards
- Add tests for new functionality
- Update documentation as needed
- Run linters before committing

### 3. Create Pull Request

```bash
# Create a draft PR early for visibility
gh pr create --draft --title "feat: description" --body "Work in progress"

# Mark ready when complete
gh pr ready
```

### PR Requirements

Your PR should include:

- **Clear description**: What does it do and why?
- **Linked issue**: Reference any related issues
- **Tests**: Unit tests and/or golden tests
- **Documentation**: Update docs if behavior changes
- **Validation**: Show it works (commands, outputs)

Example PR body:

```markdown
## Summary

Add support for recording filesystem operations.

Fixes #42

## Changes

- Extended TraceEvent with filesystem operations
- Implemented recording in CtxRecorder
- Implemented replay in CtxPlayback
- Added golden test for file I/O

## Testing

\```bash
# Build and run tests
cargo test

# Test with example component
nix build .#wasm-examples
cargo run -- record result/file_io.wasm -t fs-test.json
cargo run -- replay result/file_io.wasm fs-test.json
\```

## Breaking Changes

None
```

## Adding New Features

### Adding WASI Interface Support

See the [Architecture](./architecture.md#adding-new-wasi-interfaces) page for detailed steps.

Quick checklist:

1. [ ] Extend `TraceEvent` enum
2. [ ] Implement in `CtxRecorder`
3. [ ] Implement in `CtxPlayback`
4. [ ] Add to linker configuration
5. [ ] Write unit tests
6. [ ] Add golden test example
7. [ ] Update documentation

### Adding Examples

To add a new WASM example:

1. Create `examples/<name>/` directory
2. Add source code and build configuration
3. Update `flake.nix` examples list
4. Update `nix/golden-test.sh` to resolve the WASM path
5. Record golden fixture:
   ```bash
   nix build .#<name>-wasm
   nix run .#golden-fixture -- <name>
   ```

## Getting Help

- **Questions**: Open a [GitHub Discussion](https://github.com/acidvm/wasm-rr/discussions)
- **Bugs**: Open a [GitHub Issue](https://github.com/acidvm/wasm-rr/issues)
- **Ideas**: Open an issue with the `enhancement` label

## Code of Conduct

Be respectful, constructive, and collaborative. We're all here to make `wasm-rr` better!
