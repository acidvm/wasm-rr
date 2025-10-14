# WASI Preview2 Component Support

This document tracks the support status of WASI Preview2 components in wasm-rr.

## Legend

- ✅ **Intercepted**: Custom implementation that records/replays values for determinism
- ➡️ **Passthrough**: Uses default wasmtime-wasi implementation
- ❌ **Unsupported**: Not implemented or linked

## Component Status

### wasi:io
- ➡️ `wasi:io/streams` - Passthrough
- ➡️ `wasi:io/error` - Passthrough
- ➡️ `wasi:io/poll` - Passthrough (implicit via streams)

### wasi:clocks
- ✅ `wasi:clocks/wall-clock` - **Intercepted** (records/replays `now()` and `resolution()`)
- ➡️ `wasi:clocks/monotonic-clock` - Passthrough

### wasi:random
- ✅ `wasi:random/random` - **Intercepted** (records/replays `get_random_bytes()` and `get_random_u64()`)
- ✅ `wasi:random/insecure` - **Intercepted** (via random/random)
- ✅ `wasi:random/insecure-seed` - **Intercepted** (via random/random)

### wasi:filesystem
- ➡️ `wasi:filesystem/types` - Passthrough
- ➡️ `wasi:filesystem/preopens` - Passthrough

### wasi:cli
- ✅ `wasi:cli/environment` - **Intercepted** (records/replays `get_environment()`, `get_arguments()`, `initial_cwd()`)
- ➡️ `wasi:cli/stdin` - Passthrough
- ➡️ `wasi:cli/stdout` - Passthrough
- ➡️ `wasi:cli/stderr` - Passthrough
- ➡️ `wasi:cli/exit` - Passthrough
- ➡️ `wasi:cli/run` - Passthrough (entry point)

### wasi:sockets
- ❌ `wasi:sockets/tcp` - **Unsupported**
- ❌ `wasi:sockets/udp` - **Unsupported**
- ❌ `wasi:sockets/ip-name-lookup` - **Unsupported**
- ❌ `wasi:sockets/network` - **Unsupported**
- ❌ `wasi:sockets/instance-network` - **Unsupported**

### wasi:http
- ❌ `wasi:http/types` - **Unsupported**
- ❌ `wasi:http/outgoing-handler` - **Unsupported**
- ❌ `wasi:http/incoming-handler` - **Unsupported**

## Implementation Details

### Intercepted Components

Components marked as intercepted have custom host implementations that:
1. Call the underlying wasmtime-wasi implementation
2. Record the returned values during recording mode
3. Return recorded values during playback mode

This ensures deterministic replay of non-deterministic operations.

### Passthrough Components

Components marked as passthrough use the default wasmtime-wasi implementations directly without any recording/replay logic. These are typically deterministic operations or I/O operations that don't need replay.

### Adding New Intercepted Components

To intercept a new WASI component, follow the pattern in `src/recorder.rs` and `src/playback.rs`:
1. Add the trace event type to `TraceEvent` enum
2. Implement the Host trait with recording logic
3. Add the component to the linker with `Intercept<T>` marker
4. Implement corresponding playback logic

See [AGENTS.md](AGENTS.md) for detailed implementation guidelines.