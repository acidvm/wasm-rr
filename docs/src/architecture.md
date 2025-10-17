# Architecture

This page explains the internal architecture of `wasm-rr` for contributors and those interested in how it works.

## Overview

`wasm-rr` is built on top of [Wasmtime](https://wasmtime.dev/), a WebAssembly runtime. It implements a record-replay system by intercepting WASI host calls and either recording them or replaying previously recorded values.

## Core Components

### 1. CLI Entry Point (`src/main.rs`)

The main entry point handles:

- Command-line parsing using `clap`
- Dispatching to record, replay, or convert modes
- WASM component loading and execution
- Wasmtime engine and linker configuration

### 2. Recorder (`src/recorder.rs`)

The `CtxRecorder` wraps the WASI context and intercepts host calls:

```rust
pub struct CtxRecorder {
    wasi: WasiCtx,
    http: WasiHttpCtx,
    recorder: Recorder,
}
```

For each WASI interface (clocks, random, http, etc.), it:

1. Calls the real host implementation
2. Captures the return value
3. Records it as a `TraceEvent`
4. Returns the value to the WASM component

### 3. Playback (`src/playback.rs`)

The `CtxPlayback` replays recorded traces:

```rust
pub struct CtxPlayback {
    wasi: WasiCtx,
    http: WasiHttpCtx,
    playback: Playback,
}
```

For each WASI interface, it:

1. Reads the next event from the trace
2. Validates it matches the expected call
3. Returns the recorded value to the WASM component

### 4. Trace Events (`src/main.rs`)

The `TraceEvent` enum defines all recordable events:

```rust
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "call", rename_all = "snake_case")]
pub enum TraceEvent {
    ClockNow { seconds: u64, nanoseconds: u32 },
    RandomBytes { bytes: Vec<u8> },
    HttpResponse { /* ... */ },
    // ... more variants
}
```

## WASI Interface Integration

`wasm-rr` intercepts WASI interfaces using the host trait pattern:

```rust
// Example: wasi:random/random interface
impl random::random::Host for CtxRecorder {
    fn get_random_bytes(&mut self, len: u64) -> Result<Vec<u8>> {
        // Call the real implementation
        let bytes = self.wasi.table().random().get_random_bytes(len)?;

        // Record the event
        self.recorder.record(TraceEvent::RandomBytes {
            bytes: bytes.clone()
        });

        // Return to WASM component
        Ok(bytes)
    }
}
```

### Intercepted Interfaces

Currently intercepted WASI interfaces:

- `wasi:clocks/wall-clock` - System time
- `wasi:clocks/monotonic-clock` - Monotonic time
- `wasi:random/random` - Random number generation
- `wasi:cli/environment` - Environment variables
- `wasi:cli/run` - Command-line arguments
- `wasi:http/outgoing-handler` - HTTP requests

## Linker Configuration

The linker is configured to use our custom implementations:

```rust
// Use the Intercept pattern to provide custom host implementations
struct Intercept<T>(PhantomData<T>);

impl<T: 'static> HasData for Intercept<T> {
    type Data<'a> = &'a mut T where T: 'a;
}

// Add our intercepted interfaces
clocks::wall_clock::add_to_linker::<_, Intercept<T>>(&mut linker, |ctx| ctx)?;
random::random::add_to_linker::<_, Intercept<T>>(&mut linker, |ctx| ctx)?;
```

Non-intercepted WASI interfaces (like `wasi:io` and `wasi:filesystem`) are added normally and pass through to the real implementation.

## Trace Serialization

Traces are serialized using:

- **JSON**: Via `serde_json` with pretty printing
- **CBOR**: Via `ciborium` with streaming support

### CBOR Streaming

CBOR traces use streaming to handle large files efficiently:

```rust
// Write events one at a time
for event in events {
    ciborium::into_writer(&event, &mut writer)?;
}

// Read events until EOF
loop {
    match ciborium::from_reader(&mut reader) {
        Ok(event) => events.push(event),
        Err(e) if is_cbor_eof(&e) => break,
        Err(e) => return Err(e),
    }
}
```

## Error Handling

All errors use `anyhow::Result` with context:

```rust
let trace = File::open(trace_path)
    .with_context(|| format!("failed to open trace file at {}", trace_path.display()))?;
```

## Testing Strategy

### Unit Tests

Inline tests in `src/main.rs` using property-based testing:

```rust
#[cfg(test)]
mod tests {
    use quickcheck_macros::quickcheck;

    #[quickcheck]
    fn roundtrip_json_to_cbor_to_json(trace: TestTraceFile) -> Result<bool> {
        // Test JSON → CBOR → JSON roundtrip
    }
}
```

### Golden Tests

Located in `golden/*/`:

- `metadata.toml` - Test configuration
- `trace.json` - Expected trace
- `stdout.txt` - Expected stdout
- `stderr.txt` - Expected stderr

Run with: `nix run .#golden-test`

## Adding New WASI Interfaces

To add support for a new WASI interface:

1. **Extend `TraceEvent`**:
   ```rust
   pub enum TraceEvent {
       // ... existing variants
       NewOperation { data: String },
   }
   ```

2. **Implement in `CtxRecorder`**:
   ```rust
   impl new_interface::Host for CtxRecorder {
       fn operation(&mut self) -> Result<String> {
           let data = self.wasi.operation()?;
           self.recorder.record(TraceEvent::NewOperation {
               data: data.clone()
           });
           Ok(data)
       }
   }
   ```

3. **Implement in `CtxPlayback`**:
   ```rust
   impl new_interface::Host for CtxPlayback {
       fn operation(&mut self) -> Result<String> {
           match self.playback.next()? {
               TraceEvent::NewOperation { data } => Ok(data),
               _ => bail!("trace mismatch"),
           }
       }
   }
   ```

4. **Add to linker** in `configure_engine_and_linker()`:
   ```rust
   new_interface::add_to_linker::<_, Intercept<T>>(&mut linker, |ctx| ctx)?;
   ```

5. **Add tests** and golden fixtures

## Build System

### Cargo

Standard Rust project with:
- Main binary: `wasm-rr`
- Helper binary: `gen-cli-docs` (generates CLI reference)

### Nix Flake

The `flake.nix` provides:

- Dev shell with Rust toolchain
- WASM component examples built from multiple languages
- Golden test infrastructure
- CLI tool package

Build examples: `nix build .#wasm-examples`

## Future Work

See the project [GitHub issues](https://github.com/acidvm/wasm-rr/issues) for planned features:

- Filesystem operations recording
- Socket operations recording
- Thread synchronization recording
- Performance optimizations
- Trace compression
