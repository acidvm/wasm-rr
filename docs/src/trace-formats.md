# Trace Formats

`wasm-rr` supports two trace formats: JSON and CBOR. This page explains both formats and when to use each.

## JSON Format

JSON is the default trace format. It's human-readable and easy to inspect.

### Example JSON Trace

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
    },
    {
      "call": "http_response",
      "request_method": "GET",
      "request_url": "https://api.example.com/data",
      "request_headers": [
        ["User-Agent", "wasm-component/1.0"]
      ],
      "status": 200,
      "headers": [
        ["Content-Type", "application/json"]
      ],
      "body": [123, 34, 107, 101, 121, 34, 58, 34, 118, 97, 108, 117, 101, 34, 125]
    }
  ]
}
```

### When to Use JSON

- **Debugging**: Easy to read and understand
- **Version control**: Text diffs work well
- **Small traces**: File size isn't a concern
- **Sharing**: Can be viewed in any text editor

## CBOR Format

CBOR (Concise Binary Object Representation) is a binary format that's more compact than JSON.

### When to Use CBOR

- **Large traces**: HTTP responses with big bodies
- **Performance**: Faster to parse and write
- **Storage**: Significantly smaller file size
- **Production**: When human readability isn't needed

### Size Comparison

For typical traces, CBOR can be 30-50% smaller than JSON:

```bash
# Record the same execution in both formats
$ wasm-rr record app.wasm -t trace.json -f json
$ wasm-rr record app.wasm -t trace.cbor -f cbor

# Compare sizes
$ ls -lh trace.*
-rw-r--r-- 1 user user 245K trace.json
-rw-r--r-- 1 user user 123K trace.cbor
```

## Format Conversion

You can convert between formats using the `convert` command:

```bash
# JSON to CBOR
wasm-rr convert trace.json trace.cbor

# CBOR to JSON
wasm-rr convert trace.cbor trace.json

# Explicit format specification
wasm-rr convert input.bin output.txt --input-format cbor --output-format json
```

## Trace Event Types

Both formats support the same event types:

### Clock Events

```json
{
  "call": "clock_now",
  "seconds": 1704067200,
  "nanoseconds": 123456789
}
```

```json
{
  "call": "clock_resolution",
  "seconds": 0,
  "nanoseconds": 1
}
```

```json
{
  "call": "monotonic_clock_now",
  "nanoseconds": 987654321000
}
```

```json
{
  "call": "monotonic_clock_resolution",
  "nanoseconds": 1000
}
```

### Random Events

```json
{
  "call": "random_bytes",
  "bytes": [42, 17, 99, 3, 255]
}
```

```json
{
  "call": "random_u64",
  "value": 9223372036854775807
}
```

### Environment Events

```json
{
  "call": "environment",
  "entries": [
    ["USER", "alice"],
    ["HOME", "/home/alice"]
  ]
}
```

```json
{
  "call": "arguments",
  "args": ["app.wasm", "--config", "prod"]
}
```

```json
{
  "call": "initial_cwd",
  "path": "/home/alice/projects"
}
```

### HTTP Events

```json
{
  "call": "http_response",
  "request_method": "POST",
  "request_url": "https://api.example.com/users",
  "request_headers": [
    ["Content-Type", "application/json"],
    ["Authorization", "Bearer token123"]
  ],
  "status": 201,
  "headers": [
    ["Content-Type", "application/json"],
    ["X-Request-ID", "abc-123"]
  ],
  "body": [...]
}
```

## Format Auto-Detection

`wasm-rr` automatically detects the format based on file extension:

- `.json` → JSON format
- `.cbor` → CBOR format
- Other extensions require explicit `--format` flag

## Best Practices

1. **Use JSON for development**: Easier to inspect and debug
2. **Use CBOR for production**: Better performance and smaller size
3. **Convert as needed**: Use `convert` command when switching contexts
4. **Version control JSON**: Text-based diffs are more useful
5. **Archive as CBOR**: Save storage space for long-term archives
