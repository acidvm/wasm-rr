# Limitations

This page documents known limitations and workarounds for `wasm-rr`.

## What's Not Supported

### Filesystem Operations

**Status:** Not yet implemented

Filesystem reads and writes are not currently recorded or replayed.

**Workaround:** Keep components stateless or use environment variables/arguments for configuration.

**Planned:** Full filesystem recording is planned for a future release.

### Socket Operations

**Status:** Not yet implemented

Direct socket operations (TCP/UDP) are not recorded.

**Workaround:** Use HTTP instead when possible (HTTP is fully supported).

**Planned:** Socket recording is planned for a future release.

### Standard Input (stdin)

**Status:** Not yet implemented

Interactive stdin is not recorded.

**Impact:** Components that read from stdin during execution cannot be replayed deterministically.

**Workaround:** Pass data via arguments or files instead of stdin.

### Threading and Synchronization

**Status:** Not yet implemented

Multi-threaded components with thread spawning and synchronization primitives are not fully supported.

**Impact:** If threads depend on timing or non-deterministic scheduling, replay may differ.

**Planned:** Thread recording is planned for a future release.

## Component Requirements

### Must Be Identical for Replay

The WebAssembly component used for replay **must be identical** to the one used for recording.

**Why:** The trace records the sequence of host calls. If the component's logic changes, it may make calls in a different order or make different calls entirely.

**Impact:** Modifying the component after recording will cause replay to fail with a trace mismatch error.

**Workaround:** Keep the original component alongside its trace, or rebuild the same version from source.

### Must Use WASI Preview 2

`wasm-rr` only supports **WebAssembly components** using **WASI Preview 2** (wasip2).

**Impact:**
- Older WASI Preview 1 modules are not supported
- Core WebAssembly modules without WASI are not supported

**Workaround:** Use `wasm-tools` to convert Preview 1 modules to Preview 2 components:

```bash
wasm-tools component new module.wasm --adapt adapter.wasm -o component.wasm
```

## Trace File Compatibility

### Format Must Match

JSON and CBOR traces have the same content but different encoding.

**Impact:** You cannot directly use a trace with the wrong reader.

**Workaround:** Use the `convert` command:

```bash
wasm-rr convert trace.cbor trace.json
```

### No Version Compatibility Guarantees Yet

The trace format may change between versions of `wasm-rr`.

**Impact:** Traces recorded with an older version may not work with a newer version.

**Workaround:** Keep the version of `wasm-rr` that generated the trace, or re-record traces after upgrading.

**Future:** Once wasm-rr reaches 1.0, trace format stability will be guaranteed.

## Performance Considerations

### Large HTTP Responses

Recording components that fetch large HTTP responses will create large trace files.

**Impact:** Trace files can become very large (hundreds of MB or more).

**Workaround:**
- Use CBOR format (30-50% smaller)
- Limit the data fetched if possible
- Consider mocking large responses

### Many Random Calls

Components that generate many random values will create large traces.

**Impact:** Each random call adds an event to the trace.

**Workaround:** Use CBOR format for better compression.

## Known Issues

### Exit Codes

Component exit codes are handled but not explicitly recorded in traces.

**Impact:** A component that exits with a specific code will replay with the same code, but this isn't visible in the trace file.

### Error Messages

When replay fails due to trace mismatch, error messages show expected vs actual event types but not full context.

**Impact:** Debugging trace mismatches can be difficult.

**Workaround:** Use JSON traces and inspect them manually to see the sequence of events.

## Workarounds for Common Scenarios

### Testing with External Services

**Problem:** Tests depend on external HTTP services.

**Solution:** Record a trace with the live service, then replay in tests without network access.

```bash
# Record once with real service
wasm-rr record test.wasm -t baseline.json

# Replay in CI without network
wasm-rr replay test.wasm baseline.json
```

### Time-Dependent Tests

**Problem:** Tests behave differently at different times of day.

**Solution:** Record traces at different times and replay deterministically.

```bash
# Record morning behavior
wasm-rr record app.wasm -t morning.json

# Record evening behavior
wasm-rr record app.wasm -t evening.json

# Test both scenarios deterministically
wasm-rr replay app.wasm morning.json
wasm-rr replay app.wasm evening.json
```

### Flaky Tests

**Problem:** Tests pass sometimes and fail others.

**Solution:** Run until failure, then record the failing trace.

```bash
# Record the failure
while wasm-rr record test.wasm -t attempt.json; do
  echo "Passed, trying again..."
done

# Now debug the failure
wasm-rr replay test.wasm attempt.json
```

## Future Improvements

Planned features to address current limitations:

1. **Filesystem recording** – Record all file operations
2. **Socket recording** – Record TCP/UDP operations
3. **stdin recording** – Record interactive input
4. **Thread recording** – Deterministic multi-threading
5. **Trace format versioning** – Backward compatibility
6. **Better error messages** – More context on failures
7. **Trace compression** – Smaller trace files
8. **Partial replay** – Skip to specific points in trace

## Reporting Issues

Found a limitation not listed here? Please [open an issue](https://github.com/acidvm/wasm-rr/issues) on GitHub.
