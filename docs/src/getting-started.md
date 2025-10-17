# Getting Started

This guide will help you install `wasm-rr` and record your first trace.

## Installation

### Using Nix (Recommended)

If you have [Nix](https://docs.determinate.systems) with flakes enabled:

```bash
# Build wasm-rr
nix build github:acidvm/wasm-rr

# The binary will be at ./result/bin/wasm-rr
./result/bin/wasm-rr --version
```

Or run directly without installing:

```bash
nix run github:acidvm/wasm-rr -- --help
```

### Building from Source

Requirements:
- [Rust](https://rustup.rs) toolchain (specified in `rust-toolchain.toml`)
- Git

```bash
# Clone the repository
git clone https://github.com/acidvm/wasm-rr.git
cd wasm-rr

# Build the project
cargo build --release

# The binary will be at target/release/wasm-rr
./target/release/wasm-rr --version
```

## Your First Recording

Let's record a simple WebAssembly component. For this example, we'll assume you have a component called `app.wasm`.

### 1. Record an Execution

```bash
wasm-rr record app.wasm
```

This will:
- Run your component normally
- Capture all non-deterministic operations
- Save them to `wasm-rr-trace.json`

If your component takes arguments:

```bash
wasm-rr record app.wasm -- arg1 arg2 --flag
```

Everything after `--` is passed to your component.

### 2. Replay the Execution

```bash
wasm-rr replay app.wasm
```

This will:
- Run your component again
- Use the recorded values instead of making real host calls
- Produce exactly the same output

### 3. Verify Determinism

Run replay multiple times to confirm it's deterministic:

```bash
wasm-rr replay app.wasm > output1.txt
wasm-rr replay app.wasm > output2.txt
diff output1.txt output2.txt
# No output means they're identical!
```

## Working with Trace Files

### Custom Trace File Names

Save traces with descriptive names:

```bash
# Record different scenarios
wasm-rr record app.wasm -t success-case.json
wasm-rr record app.wasm -t error-case.json

# Replay a specific trace
wasm-rr replay app.wasm success-case.json
```

### Compact Binary Format

For smaller trace files, use CBOR format:

```bash
# Record as CBOR (file extension determines format)
wasm-rr record app.wasm -t trace.cbor

# Or explicitly specify format
wasm-rr record app.wasm -t trace.bin -f cbor

# Replay works the same way
wasm-rr replay app.wasm trace.cbor
```

CBOR traces are typically 30-50% smaller than JSON.

### Converting Between Formats

```bash
# JSON to CBOR
wasm-rr convert trace.json trace.cbor

# CBOR to JSON (for inspection)
wasm-rr convert trace.cbor trace.json
```

## Common Workflows

### Debugging a Flaky Bug

```bash
# Run your component until it fails
while wasm-rr record app.wasm -t attempt.json; do
  echo "Success, trying again..."
done

# Now you have the failure recorded
echo "Failure captured in attempt.json!"

# Debug it by replaying
wasm-rr replay app.wasm attempt.json
```

### Sharing a Bug Report

```bash
# Record the bug
wasm-rr record buggy-app.wasm -t bug-report.json

# Share both files
tar czf bug-report.tar.gz buggy-app.wasm bug-report.json

# Your teammate can reproduce it
tar xzf bug-report.tar.gz
wasm-rr replay buggy-app.wasm bug-report.json
```

### Regression Testing

```bash
# Record good behavior
wasm-rr record app.wasm -t baseline.json

# After making changes, verify behavior matches
wasm-rr replay app.wasm baseline.json > new-output.txt
# If replay succeeds, behavior is compatible!
```

## Next Steps

- Learn more about [Recording & Replaying](./recording-replaying.md)
- Understand [Trace Formats](./trace-formats.md)
- Check [Limitations](./limitations.md) for known issues
