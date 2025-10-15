# wasm-rr 🎬

**Record and replay WebAssembly programs, making the non-deterministic deterministic**

Ever had a bug that only shows up on Tuesdays during a full moon? Or spent hours trying to reproduce that one-in-a-thousand race condition your fuzzer found at 3 AM? Meet `wasm-rr` – your time-travel debugger for WebAssembly components.

## What's This All About?

WebAssembly programs can be sneaky. They read the clock, pull random numbers, fetch environment variables, make HTTP requests – all sorts of non-deterministic shenanigans that make bugs hard to catch and even harder to reproduce.

`wasm-rr` captures *everything* during program execution and lets you replay it perfectly, every single time. Think of it as a DVR for your WASM programs. Found a bug through fuzzing? Record it once, debug it forever.

## The Magic ✨

Here's what we capture and replay:
- ⏰ **Time** – Clock reads always return the same values
- 🎲 **Randomness** – Random numbers become predictably random
- 🌍 **Environment** – Variables, arguments, working directory
- 🌐 **HTTP** – Network requests and responses
- 📁 **Filesystem** – (Coming soon!)

## Quick Demo

Let's say you have a WebAssembly component that's misbehaving:

```bash
# Record the execution (creates wasm-rr-trace.json)
cargo run -- record buggy.wasm -- some arguments

# Now replay it exactly as it happened
cargo run -- replay buggy.wasm

# Or save traces for different scenarios
cargo run -- record app.wasm -t good-run.json -- --config prod
cargo run -- record app.wasm -t bad-run.json -- --config test
```

## Real-World Example: Catching Time Bugs

Imagine a WASM component that behaves differently based on the time:

```rust
// Your WebAssembly component
fn process() {
    let now = get_current_time();
    if now.hour() == 13 && now.minute() == 37 {
        panic!("🔥 Everything is on fire!");
    }
    println!("All good at {:?}", now);
}
```

Without `wasm-rr`, you'd need to wait for 1:37 PM or mess with your system clock. With `wasm-rr`:

```bash
# Capture the bug when it happens
cargo run -- record time-bomb.wasm -t bug-at-1337.json

# Replay it anytime, anywhere
cargo run -- replay time-bomb.wasm bug-at-1337.json
# 💥 Panic at 13:37 every time!
```

## Golden Testing 🏆

We use "golden" tests to ensure our record-replay mechanism works perfectly. Each test captures not just the trace, but also stdout/stderr output:

```bash
# Build everything
nix build .

# Record a new golden test fixture
nix run .#golden-fixture -- print_random

# Run all golden tests
nix run .#golden-test
```

Golden fixtures live in `golden/<component>/` with:
- `trace.json` – The recorded execution
- `stdout.txt` – Expected standard output
- `stderr.txt` – Expected error output
- `metadata.toml` – Test configuration

## Get Started

### Prerequisites

- Rust toolchain (we pin to a specific version via `rust-toolchain.toml`)
- Nix (optional, but recommended for reproducible builds)

### Building

```bash
# Using Cargo
cargo build --release

# Using Nix (recommended)
nix build .
```

### Running Examples

We include several example WASM components to play with:

```bash
# See what time the WASM component thinks it is
cargo run -- record examples/print_time/print_time.wasm
cargo run -- replay examples/print_time/print_time.wasm

# Watch random numbers become deterministic
cargo run -- record examples/print_random/print_random.wasm
cargo run -- replay examples/print_random/print_random.wasm

# Test with arguments
cargo run -- record examples/print_args/print_args.wasm -- hello world
```

## How It Works

`wasm-rr` sits between your WebAssembly component and the host runtime, intercepting all non-deterministic host calls:

1. **Recording Mode**: We run your WASM component normally but capture every non-deterministic operation into a trace file
2. **Replay Mode**: We run the same component but instead of making real host calls, we return the recorded values

This approach means:
- ✅ Perfect reproduction of bugs found through fuzzing
- ✅ Time-travel debugging without special tooling
- ✅ Shareable bug reports (just send the trace file!)
- ✅ Deterministic testing of non-deterministic code

## Project Status

Currently supported:
- ✅ Clock/time operations (`wasi:clocks`)
- ✅ Random number generation (`wasi:random`)
- ✅ Environment variables and arguments (`wasi:cli`)
- ✅ HTTP requests/responses (`wasi:http`)

Coming soon:
- 🚧 Filesystem operations (`wasi:filesystem`)
- 🚧 Socket operations (`wasi:sockets`)
- 🚧 Thread spawning and synchronization

## Contributing

Found a bug? Have an idea? We'd love your help! Check out [AGENTS.md](AGENTS.md) for our development guidelines and workflow.

### Quick Development Commands

```bash
# Run tests
cargo test

# Format and lint
cargo fmt && cargo clippy --all-targets --all-features

# Run golden tests
nix run .#golden-test

# Record new golden fixture
nix run .#golden-fixture -- <component-name>
```

## License

[Add your license here]

## Why "wasm-rr"?

**W**eb**A**ssembly **S**eamless **M**agic **R**ecord-**R**eplay... okay, we just liked how it sounded. 🎭

---

*Built with 🦀 Rust and a sprinkle of time-travel magic*