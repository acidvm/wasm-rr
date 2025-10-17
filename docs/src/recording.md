# Recording Execution

This page explains how to record WebAssembly component execution with `wasm-rr`.

## Basic Recording

The simplest way to record is:

```bash
wasm-rr record component.wasm
```

This creates a `wasm-rr-trace.json` file in the current directory containing all non-deterministic host calls.

## Specifying the Trace File

You can specify a custom trace file path:

```bash
wasm-rr record component.wasm -t my-trace.json
wasm-rr record component.wasm --trace my-trace.json
```

## Trace Formats

`wasm-rr` supports two trace formats:

- **JSON** (default): Human-readable, easy to inspect
- **CBOR**: Compact binary format, smaller file size

The format is automatically detected from the file extension:

```bash
# JSON format (explicit extension)
wasm-rr record component.wasm -t trace.json

# CBOR format (explicit extension)
wasm-rr record component.wasm -t trace.cbor
```

You can also explicitly specify the format:

```bash
wasm-rr record component.wasm -t trace.bin -f cbor
```

## Passing Arguments to the Component

To pass arguments to your WebAssembly component, use `--` to separate wasm-rr arguments from component arguments:

```bash
wasm-rr record app.wasm -- --config prod --verbose
```

Everything after `--` is passed to the component.

## What Gets Recorded?

During recording, `wasm-rr` captures:

- **Clock operations**: All `wasi:clocks` calls (wall clock and monotonic clock)
- **Random values**: All `wasi:random` calls
- **Environment**: Environment variables, command-line arguments, initial working directory
- **HTTP requests**: Full request and response data from `wasi:http` calls

The trace file contains all these values in the exact order they were requested by the component.

## Example Recording Session

```bash
# Record a component that makes HTTP requests
$ wasm-rr record fetch_quote.wasm -t http-example.json
Fetching quote from API...
Quote: "The only way to do great work is to love what you do." - Steve Jobs

# Inspect the trace (JSON is human-readable!)
$ cat http-example.json
{
  "events": [
    {
      "call": "clock_now",
      "seconds": 1704067200,
      "nanoseconds": 123456789
    },
    {
      "call": "http_response",
      "request_method": "GET",
      "request_url": "https://api.quotable.io/random",
      "status": 200,
      "body": [...]
    }
  ]
}
```

## Tips

- **Keep traces small**: Record only what you need for debugging
- **Use CBOR for large traces**: The binary format is much more compact
- **Version control traces**: Trace files are great for regression tests
- **Share traces**: Send trace files to teammates for bug reproduction
