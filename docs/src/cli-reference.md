# wasm-rr CLI Reference

This page contains the auto-generated reference documentation for the `wasm-rr` command-line interface.

# Command-Line Help for `wasm-rr`

This document contains the help content for the `wasm-rr` command-line program.

**Command Overview:**

* [`wasm-rr`↴](#wasm-rr)
* [`wasm-rr record`↴](#wasm-rr-record)
* [`wasm-rr replay`↴](#wasm-rr-replay)
* [`wasm-rr convert`↴](#wasm-rr-convert)

## `wasm-rr`

**Usage:** `wasm-rr [COMMAND]`

###### **Subcommands:**

* `record` — Record all non-deterministic host calls while running the component
* `replay` — Replay previously recorded host calls from a trace file
* `convert` — Convert a trace file between JSON and CBOR formats



## `wasm-rr record`

Record all non-deterministic host calls while running the component

**Usage:** `wasm-rr record [OPTIONS] <WASM> [ARGS]...`

###### **Arguments:**

* `<WASM>` — Path to the component to execute
* `<ARGS>` — Arguments to forward to the component (use `--` to separate)

###### **Options:**

* `-t`, `--trace <TRACE>` — Output file for the trace (extension determines format: .json or .cbor)

  Default value: `wasm-rr-trace.json`
* `-f`, `--format <FORMAT>` — Trace format (json or cbor). If not specified, inferred from file extension

  Possible values: `json`, `cbor`




## `wasm-rr replay`

Replay previously recorded host calls from a trace file

**Usage:** `wasm-rr replay [OPTIONS] <WASM> [TRACE]`

###### **Arguments:**

* `<WASM>` — Path to the component to execute
* `<TRACE>` — Input trace file (extension determines format: .json or .cbor)

  Default value: `wasm-rr-trace.json`

###### **Options:**

* `-f`, `--format <FORMAT>` — Trace format (json or cbor). If not specified, inferred from file extension

  Possible values: `json`, `cbor`




## `wasm-rr convert`

Convert a trace file between JSON and CBOR formats

**Usage:** `wasm-rr convert [OPTIONS] <INPUT> <OUTPUT>`

###### **Arguments:**

* `<INPUT>` — Input trace file
* `<OUTPUT>` — Output trace file (extension determines format: .json or .cbor)

###### **Options:**

* `--input-format <FORMAT>` — Input format (json or cbor). If not specified, inferred from file extension

  Possible values: `json`, `cbor`

* `--output-format <FORMAT>` — Output format (json or cbor). If not specified, inferred from file extension

  Possible values: `json`, `cbor`




<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

