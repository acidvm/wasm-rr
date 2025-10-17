# Trace Formats

`wasm-rr` supports two trace formats: JSON and CBOR.

## JSON Format (Default)

JSON traces are human-readable text files.

**Advantages:**
- Easy to inspect and understand
- Works with standard text tools (grep, jq, etc.)
- Good for version control (meaningful diffs)
- Can be viewed in any text editor

**Use JSON when:**
- Debugging or inspecting traces
- Storing traces in version control
- File size isn't a concern

**Example:**

```bash
wasm-rr record app.wasm -t trace.json
```

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
      "call": "environment",
      "entries": [
        ["HOME", "/home/user"],
        ["PATH", "/usr/bin:/bin"]
      ]
    }
  ]
}
```

## CBOR Format (Binary)

CBOR (Concise Binary Object Representation) is a compact binary format.

**Advantages:**
- 30-50% smaller than JSON
- Faster to read and write
- Good for large traces

**Use CBOR when:**
- Traces are large (lots of HTTP responses)
- Storage space matters
- You don't need to inspect the trace manually

**Example:**

```bash
wasm-rr record app.wasm -t trace.cbor
```

### Size Comparison

```bash
$ wasm-rr record app.wasm -t trace.json
$ wasm-rr record app.wasm -t trace.cbor
$ ls -lh trace.*
-rw-r--r-- 1 user user 245K trace.json
-rw-r--r-- 1 user user 123K trace.cbor  # 50% smaller!
```

## Converting Between Formats

Use the `convert` command to switch formats:

```bash
# Make a trace human-readable
wasm-rr convert trace.cbor trace.json

# Make a trace more compact
wasm-rr convert trace.json trace.cbor

# Explicit format specification
wasm-rr convert input.bin output.json --input-format cbor --output-format json
```

## Format Detection

The format is automatically detected from the file extension:

| Extension | Format |
|-----------|--------|
| `.json`   | JSON   |
| `.cbor`   | CBOR   |

For other extensions, specify the format explicitly:

```bash
wasm-rr record app.wasm -t trace.bin -f cbor
wasm-rr replay app.wasm trace.bin -f cbor
```

## What's In a Trace?

Both formats store the same information - just encoded differently.

### Event Types

**Clock operations:**
- `clock_now` – Wall clock time
- `clock_resolution` – Clock precision
- `monotonic_clock_now` – Monotonic time
- `monotonic_clock_resolution` – Monotonic clock precision

**Random values:**
- `random_bytes` – Array of random bytes
- `random_u64` – Random 64-bit integer

**Environment:**
- `environment` – Environment variables
- `arguments` – Command-line arguments
- `initial_cwd` – Initial working directory

**HTTP:**
- `http_response` – Complete HTTP request and response

### Inspecting JSON Traces

Use standard tools to inspect JSON traces:

```bash
# View the trace
cat trace.json

# Pretty-print
jq . trace.json

# Count events
jq '.events | length' trace.json

# See event types
jq '.events[].call' trace.json | sort | uniq -c

# Find HTTP calls
jq '.events[] | select(.call == "http_response")' trace.json
```

## Best Practices

**For development and debugging:**
- Use JSON format
- Keep traces in version control
- Use descriptive filenames

**For production or archival:**
- Use CBOR format
- Saves significant space
- Faster to process

**General tips:**
- Convert to JSON when you need to inspect
- Convert to CBOR before archiving
- Use `.json` or `.cbor` extensions for auto-detection
