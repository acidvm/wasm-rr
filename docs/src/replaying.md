# Replaying Traces

This page explains how to replay recorded traces with `wasm-rr`.

## Basic Replay

To replay a trace:

```bash
wasm-rr replay component.wasm
```

This reads `wasm-rr-trace.json` (the default trace file) and replays the execution.

## Specifying the Trace File

You can specify a custom trace file:

```bash
wasm-rr replay component.wasm my-trace.json
wasm-rr replay component.wasm traces/bug-reproduction.cbor
```

## Format Detection

The trace format (JSON or CBOR) is automatically detected from the file extension:

- `.json` → JSON format
- `.cbor` → CBOR format

You can also explicitly specify the format:

```bash
wasm-rr replay component.wasm trace.bin -f cbor
```

## Deterministic Replay

During replay, `wasm-rr` ensures that:

1. **All non-deterministic calls return recorded values**: Time, random numbers, HTTP responses, etc.
2. **The execution order matches**: Events are replayed in the exact order they were recorded
3. **No actual host calls are made**: HTTP requests use recorded responses, time returns recorded values

This means:

- ✅ No network access needed (HTTP requests use cached responses)
- ✅ Execution is completely deterministic
- ✅ Time always returns the recorded values
- ✅ Random numbers are "predictably random"

## Trace Validation

If the component's execution diverges from the recorded trace, `wasm-rr` will fail with an error:

```bash
$ wasm-rr replay modified-component.wasm original-trace.json
Error: Trace mismatch: expected ClockNow but got RandomBytes
```

This happens when:

- The component has been modified
- The component uses different code paths based on input
- The trace file is corrupted or incompatible

## Example Replay Session

```bash
# Record a component's execution
$ wasm-rr record random-demo.wasm -t random-trace.json
Random values: 42, 17, 99, 3

# Replay it - get the exact same output!
$ wasm-rr replay random-demo.wasm random-trace.json
Random values: 42, 17, 99, 3

# Replay again - still the same!
$ wasm-rr replay random-demo.wasm random-trace.json
Random values: 42, 17, 99, 3
```

## Debugging with Replay

Replay is especially useful for debugging:

### 1. Reproduce Fuzzer Findings

```bash
# Fuzzer found a crash
$ fuzzer --output crash.wasm

# Record the crash
$ wasm-rr record crash.wasm -t crash-trace.json
[crash occurs]

# Now replay it in a debugger as many times as needed
$ gdb --args wasm-rr replay crash.wasm crash-trace.json
```

### 2. Test Fixes

```bash
# Record a bug
$ wasm-rr record buggy-v1.wasm -t bug-trace.json

# Fix the bug
$ # ... make changes ...
$ cargo build --release --target wasm32-wasip2

# Verify the fix
$ wasm-rr replay fixed-v2.wasm bug-trace.json
# If the trace validates, the fix works!
```

### 3. Share Bug Reports

```bash
# Record the problematic execution
$ wasm-rr record app.wasm -t bug-report.json

# Send both files to your team
$ tar czf bug-report.tar.gz app.wasm bug-report.json

# Teammate can replay it exactly
$ tar xzf bug-report.tar.gz
$ wasm-rr replay app.wasm bug-report.json
```

## Limitations

- **Component must be identical**: The replayed component should be the same as the recorded one
- **Input-dependent behavior**: If the component takes user input during execution (stdin), this is not yet recorded
- **Filesystem operations**: Not yet supported (coming soon!)
