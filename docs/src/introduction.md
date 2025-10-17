# Introduction

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

## Quick Example

Let's say you have a WebAssembly component that's misbehaving:

```bash
# Record the execution (creates wasm-rr-trace.json)
wasm-rr record buggy.wasm -- some arguments

# Now replay it exactly as it happened
wasm-rr replay buggy.wasm

# Or save traces for different scenarios
wasm-rr record app.wasm -t good-run.json -- --config prod
wasm-rr record app.wasm -t bad-run.json -- --config test
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

## Next Steps

- Learn about the [CLI commands](./cli-reference.md)
- Read the [Recording Execution](./recording.md) guide
- Understand [Trace Formats](./trace-formats.md)
