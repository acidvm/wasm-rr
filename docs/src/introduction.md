# Introduction

**Record and replay WebAssembly programs, making the non-deterministic deterministic**

Ever had a bug that only shows up occasionally? Or spent hours trying to reproduce that one rare condition? `wasm-rr` lets you record a WebAssembly component's execution once and replay it perfectly, every time.

## What Does wasm-rr Do?

WebAssembly programs often behave differently each time they run because they:
- Read the current time
- Generate random numbers
- Make HTTP requests
- Read environment variables

`wasm-rr` captures all these non-deterministic operations during execution and saves them to a trace file. You can then replay that exact execution as many times as you want.

## Use Cases

**Debugging**: Found a bug that's hard to reproduce? Record it once, replay it endlessly while debugging.

**Testing**: Share a trace file with your team so they can reproduce the exact same behavior.

**Fuzzing**: Capture interesting fuzzer findings and replay them deterministically.

**Regression Testing**: Keep trace files as test cases to ensure bugs don't come back.

## Quick Example

```bash
# Record a component's execution
wasm-rr record my-app.wasm

# Replay it exactly as it happened
wasm-rr replay my-app.wasm

# Save different scenarios
wasm-rr record my-app.wasm -t success.json -- --config prod
wasm-rr record my-app.wasm -t failure.json -- --config test
```

## What Gets Recorded

- ⏰ **Time** – All clock reads (wall clock and monotonic)
- 🎲 **Random numbers** – All random value generation
- 🌍 **Environment** – Variables, arguments, working directory
- 🌐 **HTTP** – Complete requests and responses

## What's Not Recorded Yet

- 📁 **Filesystem** – File operations (coming soon)
- 🔌 **Sockets** – Network operations (coming soon)
- 🧵 **Threads** – Threading and synchronization (coming soon)
- ⌨️ **Standard input** – Interactive stdin (coming soon)

## Next Steps

- [CLI Reference](./cli-reference.md) – Complete command-line reference
