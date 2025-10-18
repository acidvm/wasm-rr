# Generalized InputStream Recording
- Summary: Capture host-provided input stream data during recording and replay it deterministically without touching host IO during playback.
- Owner: Codex assistant (initial draft), wasm-rr maintainers
- Last updated: 2025-10-16

## Problem Overview

`wasm-rr` currently ignores `wasi:io/streams::input-stream` traffic, which leaves gaps when a component consumes data from `stdin`, pipes, sockets, or filesystem descriptors opened through `wasi:filesystem`. During recording we must intercept every non-deterministic byte that reaches the component so that replay mode can satisfy the same reads without delegating to host resources. The design needs to cover three scenarios:
- Preopened resources such as `stdin` and directory descriptors that hand out `input-stream` handles through WASI APIs.
- Dynamic descriptors obtained at runtime (`open-at`, sockets, pipes) whose underlying host data may differ between runs.
- Derived resources like pollables that a component creates from an input stream to wait for readiness.

The solution should treat all `input-stream` resources uniformly, so future APIs that produce streams (for example, HTTP response bodies once `wasi:http` moves to streams) automatically inherit deterministic behavior.

## Resources in Wasmtime

### Component resources and tables

Wasmtime’s component model host bindings store guest-visible handles in a `ResourceTable`. Each entry wraps a host-side object plus bookkeeping (drop callbacks, borrow tracking). Guests receive opaque indices represented as `wasmtime::component::Resource<T>`, and Wasmtime enforces that operations on a handle borrow the corresponding entry mutably or immutably. When a resource is dropped on the guest side, Wasmtime invokes the host drop hook to release the entry.

The table can hold multiple resource kinds at once; host code downcasts indices using the concrete type parameter `T`. Hosts are responsible for any extra state, such as mapping from a resource to a pollable or keeping per-handle metadata.

### Input streams in WASI

The `wasi:io/streams` WIT package defines the `input-stream` resource with non-blocking `read`, `blocking-read`, `skip`, `blocking-skip`, and `subscribe` methods, plus an implicit `drop`. When the component calls `subscribe` it receives a `pollable` child resource. Errors use the shared `stream-error` variant (`last-operation-failed` or `closed`).

Filesystem descriptors expose input streams through `wasi:filesystem/types::descriptor::read-via-stream`. Similar patterns exist (or will exist) in other interfaces, so an implementation that hooks these traits once will automatically cover every producer of `input-stream`. The host implementations of these traits live behind:
- `wasmtime_wasi::p2::bindings::io::streams::HostInputStream`
- `wasmtime_wasi::p2::bindings::sync::filesystem::types::HostDescriptor`
- `wasmtime_wasi::p2::bindings::sync::filesystem::preopens::Host`

Wasmtime forwards guest calls into the `Host*` traits, and the default CLI store (`WasiCtx`) already registers real OS handles there. To make streams deterministic we must intercept these trait calls, capture data in the recorder, and serve pre-recorded answers in playback.

## Implementation Plan

### Goals and constraints
- Capture every byte and control signal (`len` replies, EOF, error) that an `input-stream` surfaces to the guest.
- Avoid issuing host IO during replay; instead, feed bytes from the trace and surface the same `stream-error`s.
- Support multiple concurrent streams and cloned handles, preserving ordering of operations per stream.
- Keep trace format stable and backwards-compatible by adding new event variants rather than restructuring existing data.

### Extend the trace schema
1. Add `TraceEvent::InputStreamCreate { trace_id, origin }`, emitted whenever a new `input-stream` handle becomes visible to the guest. `origin` records context such as `Stdin`, `Descriptor { path, descriptor_flags }`, or `Pipe { producer }` when available; unknown sources map to `Unknown`.
2. Add `TraceEvent::InputStreamRead { trace_id, kind, payload }` where `kind` distinguishes `read`, `blocking_read`, `skip`, and `blocking_skip`. Payload includes the byte vector or skipped length plus an `eof` flag. If the host reports `stream-error`, record `TraceEvent::InputStreamError { trace_id, kind, error }`.
3. Add `TraceEvent::InputStreamSubscribe { trace_id, pollable_id }` plus `TraceEvent::PollableReady { pollable_id }` events so replay can emulate readiness transitions deterministically without touching host pollables.
4. Document the additions in `docs/trace-format.md` (or create it) and gate loading on optional presence so older traces still parse.

### Recorder integration
1. Wrap the existing WASI store in an `Intercept<CtxRecorder>` that implements `HostInputStream`, `HostDescriptor`, and any other producer traits. When `read-via-stream` (or similar) creates a stream, allocate a stable `trace_id` (monotonic counter) and stash it alongside the real host resource inside a new `RecordedInputStream` struct stored in the `ResourceTable`.
2. Implement each method of `HostInputStream` by delegating to the inner WASI implementation, then recording the resulting data/error with the appropriate trace event. For `skip` variants, record the count that the host returned; for successful `read` variants, record the byte vector and whether EOF was observed (zero-length read with `closed` flag). Propagate host errors yet log them so playback can reproduce them.
3. Intercept `subscribe` to allocate deterministic pollable IDs. The recorder should still return the real host pollable to the guest so execution proceeds, but must emit matching `PollableReady` events when the pollable resolves. Use the existing runtime integration (Tokio) to await readiness and push the event before returning readiness to the component.
4. Ensure `drop` removes mapping entries and optionally emits `TraceEvent::InputStreamDrop` (useful for debugging mismatched lifetimes).

### Playback integration
1. Build a `ReplayInputStream` struct containing the event queue for its `trace_id`, current cursor, and metadata flags. Store instances in the playback `ResourceTable` when the component first obtains the handle, using the `InputStreamCreate` events to know when to instantiate them.
2. Implement `HostInputStream` for `CtxPlayback` to service `read`/`skip` calls by consuming the next recorded event for that `trace_id`. For `read` events, return the captured byte vector; for `skip`, return the stored count; for errors, convert back into `stream-error`.
3. Implement `subscribe` by returning a synthetic pollable handle that drives completion off the recorded `PollableReady` events. One option is to back `pollable` with a channel that resolves immediately when the matching event is consumed.
4. Honor ordering: enforce that events are consumed in the same sequence they were recorded, and raise a descriptive mismatch error if playback requests deviate (wrong handle, unexpected operation).

### Resource bookkeeping
- Maintain a `StreamRegistry` in both recorder and playback contexts that maps `Resource<input-stream>` handles to `trace_id` and supplementary metadata (origin, outstanding pollables). Integrate with Wasmtime’s drop hooks to clean up automatically.
- For recorder mode, store the OS-backed stream in the registry so host operations still function; for playback, store just trace state.
- Teach descriptor-related interceptors to call into the registry whenever they produce an `input-stream`, ensuring streams created through filesystem APIs receive IDs before the guest can use them.

### Testing and validation
1. Add focused unit tests around the registry logic (mocking the event sink/source) to verify creation, read ordering, skip semantics, and error propagation.
2. Create a new example component that reads from stdin and another that uses `read-via-stream` on a file to validate both direct and indirect stream creation paths. Record golden traces and ensure replay runs without touching the filesystem or stdin.
3. Exercise pollable readiness via an integration test that uses `subscribe` and `poll::poll-oneoff` to confirm deterministic wakeups.
4. Incorporate regression tests that ensure absent events yield informative errors (e.g., truncated trace fails gracefully).

### Open questions / follow-up
- How much origin metadata can we capture without leaking host paths? Consider hashing or redacting paths for reproducibility.
- Do we need to support lazy loading of large stream payloads (e.g., files >MB) to keep trace size manageable? Possible future optimization: chunked events.
- Should pollable replay tie into the existing monotonic clock events to honor timeouts? Initial version can assume immediate readiness, but sustaining timeouts might require additional trace data later.

