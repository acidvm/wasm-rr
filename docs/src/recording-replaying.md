# Recording & Replaying

This page covers everything you need to know about recording and replaying WebAssembly component execution.

## Recording

### Basic Recording

The simplest way to record:

```bash
wasm-rr record component.wasm
```

This creates `wasm-rr-trace.json` containing all non-deterministic host calls made during execution.

### Passing Arguments

To pass arguments to your component, use `--` to separate them:

```bash
wasm-rr record app.wasm -- --config prod --verbose
```

Everything after `--` goes to your component.

### Specifying Trace Files

Choose where to save the trace:

```bash
# Custom filename
wasm-rr record app.wasm -t my-trace.json

# Different scenarios
wasm-rr record app.wasm -t test-case-1.json -- --input data1.txt
wasm-rr record app.wasm -t test-case-2.json -- --input data2.txt
```

### Choosing a Format

`wasm-rr` supports two trace formats:

**JSON** (default)
- Human-readable
- Easy to inspect
- Good for debugging
- Larger file size

```bash
wasm-rr record app.wasm -t trace.json
```

**CBOR** (binary)
- Compact (30-50% smaller)
- Faster to read/write
- Good for large traces

```bash
wasm-rr record app.wasm -t trace.cbor
```

The format is auto-detected from the file extension. You can also specify it explicitly:

```bash
wasm-rr record app.wasm -t trace.bin -f cbor
```

### What Gets Recorded?

During recording, `wasm-rr` captures:

**Time operations**
- Wall clock reads (seconds and nanoseconds)
- Monotonic clock reads
- Clock resolution queries

**Random values**
- Random bytes
- Random 64-bit integers

**Environment**
- Environment variables
- Command-line arguments
- Initial working directory

**HTTP requests**
- Request method, URL, headers
- Response status, headers, body

The trace file contains these values in the exact order they were requested.

## Replaying

### Basic Replay

To replay a trace:

```bash
wasm-rr replay component.wasm
```

This reads `wasm-rr-trace.json` (the default) and replays the execution.

### Specifying Trace Files

Replay a specific trace:

```bash
wasm-rr replay component.wasm my-trace.json
wasm-rr replay component.wasm test-case-1.json
```

The format is auto-detected from the file extension.

### How Replay Works

During replay:

1. **All non-deterministic calls return recorded values**
   - Time always returns the recorded time
   - Random numbers return the recorded random values
   - HTTP requests use cached responses (no actual network calls)

2. **Execution is completely deterministic**
   - No network access needed
   - No dependency on system time
   - Same output every time

3. **Trace validation**
   - If the component makes different calls, replay fails
   - This ensures the trace matches the component

### Trace Validation Errors

If the component has changed or uses different code paths:

```bash
$ wasm-rr replay modified-component.wasm original-trace.json
Error: Trace mismatch: expected ClockNow but got RandomBytes
```

This happens when:
- The component has been modified
- The component takes different code paths
- The trace file is corrupted

## Advanced Usage

### Inspecting JSON Traces

JSON traces are human-readable:

```bash
# View a trace
cat trace.json

# Pretty-print with jq
jq . trace.json

# Count events
jq '.events | length' trace.json

# See what types of calls were made
jq '.events[].call' trace.json | sort | uniq -c
```

Example trace structure:

```json
{
  "events": [
    {
      "call": "clock_now",
      "seconds": 1704067200,
      "nanoseconds": 123456789
    },
    {
      "call": "random_u64",
      "value": 42
    },
    {
      "call": "http_response",
      "request_method": "GET",
      "request_url": "https://api.example.com/data",
      "status": 200,
      "headers": [["Content-Type", "application/json"]],
      "body": [...]
    }
  ]
}
```

### Converting Formats

Convert between JSON and CBOR:

```bash
# JSON to CBOR (for smaller size)
wasm-rr convert trace.json trace.cbor

# CBOR to JSON (for inspection)
wasm-rr convert trace.cbor trace.json

# Explicit format specification
wasm-rr convert input.bin output.txt --input-format cbor --output-format json
```

### Version Control

**Recommended practices:**

- **Use JSON for version control** – Text diffs are meaningful
- **Use CBOR for archives** – Save space for long-term storage
- **Keep traces small** – Record only what you need
- **Name traces descriptively** – `success-case.json`, `error-case.json`

```bash
# Good for git
git add test-cases/*.json

# Convert to CBOR for archiving
for f in test-cases/*.json; do
  wasm-rr convert "$f" "${f%.json}.cbor"
done
```

## Practical Examples

### Debugging Time-Dependent Code

```bash
# Record at a specific time
wasm-rr record time-sensitive.wasm -t at-midnight.json

# Replay later - same time every time
wasm-rr replay time-sensitive.wasm at-midnight.json
```

### Testing HTTP-Dependent Code

```bash
# Record with live API
wasm-rr record api-client.wasm -t api-response.json

# Replay without network - uses recorded response
wasm-rr replay api-client.wasm api-response.json
```

### Capturing Rare Bugs

```bash
#!/bin/bash
# Record until failure
attempt=1
while wasm-rr record flaky-app.wasm -t "attempt-$attempt.json"; do
  echo "Attempt $attempt: success"
  attempt=$((attempt + 1))
done

echo "Failure captured in attempt-$attempt.json"

# Now debug it
wasm-rr replay flaky-app.wasm "attempt-$attempt.json"
```

### Regression Testing

```bash
# Record baseline behavior
wasm-rr record app-v1.wasm -t baseline.json

# After making changes
wasm-rr record app-v2.wasm -t new-version.json

# Compare outputs
wasm-rr replay app-v1.wasm baseline.json > v1-output.txt
wasm-rr replay app-v2.wasm baseline.json > v2-output.txt
diff v1-output.txt v2-output.txt
```

## Tips and Tricks

**Keep traces manageable**
- Record only what you need for the specific test case
- Use CBOR for large traces (especially with big HTTP responses)

**Name traces descriptively**
- `login-success.json`, `login-invalid-password.json`
- Include dates or versions if relevant

**Use traces as test fixtures**
- Check them into your test suite
- Replay them in CI to catch regressions

**Share traces for bug reports**
- Send both the WASM component and trace file
- Anyone can replay the exact issue

**Verify determinism**
- Replay multiple times to ensure consistency
- Use `diff` to compare outputs
