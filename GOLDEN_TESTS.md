# Golden Test Generation Guide

This document describes how to generate each golden test fixture for wasm-rr. All tests are run the same way (via `nix run .#golden-test` or `nix flake check`), but each has specific generation requirements.

## Overview

Golden tests verify that wasm-rr can record and replay WebAssembly components deterministically. Each test consists of:

- `metadata.toml` - Test configuration
- `trace.json` - Recorded trace of host calls
- `stdout.txt` - Expected standard output
- `stderr.txt` - Expected standard error

## Generation Commands

### Basic Examples (No Arguments, No Stdin)

These examples are generated using the standard golden-fixture command:

```bash
nix run .#golden-fixture -- --component <name>
```

**Examples:**
- `c_hello_world` - Simple C hello world
- `go_hello_world` - Go hello world using TinyGo
- `hello_haskell` - Haskell hello world
- `fizzbuzz_zig` - FizzBuzz implementation in Zig
- `js_wordstats` - JavaScript word statistics using Javy
- `print_time` - Prints current wall clock time
- `print_random` - Prints random bytes
- `fetch_quote` - Fetches a quote via HTTP

### Examples with Arguments

#### `print_args`

Tests command-line argument passing.

**Generation:**
```bash
nix build .#print_args-wasm
cargo run -- record result/print_args.wasm -t golden/print_args/trace.json -- hello world
cargo run -- replay result/print_args.wasm golden/print_args/trace.json > golden/print_args/stdout.txt 2> golden/print_args/stderr.txt
```

#### `bench_num`

Runs Criterion benchmarks with quick mode and discarded baseline.

**Generation:**
```bash
nix build .#bench_num-wasm
cargo run -- record result/bench_num.wasm -t golden/bench_num/trace.json -- --bench --quick --discard-baseline
cargo run -- replay result/bench_num.wasm golden/bench_num/trace.json > golden/bench_num/stdout.txt 2> golden/bench_num/stderr.txt
```

#### `counts`

Component with custom arguments (if any).

**Generation:**
```bash
nix run .#golden-fixture -- --component counts
```

### Examples with Stdin

#### `hello_python`

Python script must be provided via stdin since the CPython WASM runtime reads the script from stdin.

**Generation:**
```bash
nix build .#hello_python-wasm
cat result/app.py | cargo run -- record result/hello_python.wasm -t golden/hello_python/trace.json
cat result/app.py | cargo run -- replay result/hello_python.wasm golden/hello_python/trace.json > golden/hello_python/stdout.txt 2> golden/hello_python/stderr.txt
```

#### `read_stdin`

This test is marked as `must_fail = true` because it attempts to read from stdin during replay, which should exhaust the trace and fail deterministically.

**Generation:**
```bash
# Note: This test is expected to fail during replay
nix build .#read_stdin-wasm
echo "test input" | cargo run -- record result/read_stdin.wasm -t golden/read_stdin/trace.json
echo "test input" | cargo run -- replay result/read_stdin.wasm golden/read_stdin/trace.json > golden/read_stdin/stdout.txt 2> golden/read_stdin/stderr.txt || true
```

## Test Categories

### 1. Hello World Tests
- `c_hello_world` - C
- `go_hello_world` - Go (TinyGo)
- `hello_haskell` - Haskell
- `hello_python` - Python (requires stdin)
- `fizzbuzz_zig` - Zig

### 2. I/O Tests
- `print_args` - Command-line arguments
- `read_stdin` - Standard input (expected failure)
- `fetch_quote` - HTTP requests

### 3. Non-Determinism Tests
- `print_time` - Wall clock
- `print_random` - Random number generation

### 4. Computation Tests
- `bench_num` - Criterion benchmarks with BigInt
- `js_wordstats` - JavaScript text processing
- `counts` - Custom computation example

## Special Cases

### Expected Failures

Tests with `must_fail = true` in their metadata are expected to fail during replay. This verifies that wasm-rr correctly detects when a component attempts operations that aren't in the trace.

**Current examples:**
- `read_stdin` - Attempts to read from stdin during replay, which should fail with "trace exhausted"

### Components Requiring Stdin

Some components need their input provided via stdin. This is common for language runtimes that read source code from stdin:

- `hello_python` - CPython WASM runtime reads Python script from stdin
- `read_stdin` - Test component that reads from stdin (expected to fail)

## Metadata Format

```toml
component = "component_name"
trace = "trace.json"
stdout = "stdout.txt"
stderr = "stderr.txt"
must_fail = true  # Optional: mark test as expected to fail
```

## Adding New Golden Tests

1. Create the WASM component in `examples/<name>/`
2. Build: `nix build .#<name>-wasm`
3. Generate fixture using appropriate method from above
4. Update `flake.nix` to include the new example
5. Update `nix/golden-test.sh`:
   - Add component to `resolve_wasm()` function
   - If component needs stdin, add to `get_stdin_file()` function
6. Verify: `nix flake check`

## Notes

- Timestamps and random values will differ between recordings - this is expected
- The trace format uses hex encoding for byte arrays (e.g., `"bytes":"a1b2c3"`)
- All golden tests must pass during `nix flake check` for CI to succeed
- Use `nix run .#golden-test` to run only the golden tests without full checks
