# Repository Guidelines

## Project Structure & Module Organization
The CLI entrypoint lives in `src/main.rs`, with record/replay logic in `src/recorder.rs` and `src/playback.rs`. Sample WebAssembly components reside in `examples/`, each with its own Cargo manifest; build them via `nix build .`, which writes artifacts to `result/*.wasm`. Generated artifacts for the CLI collect in `target/`; `rust-toolchain.toml` pins the toolchain expected by Wasmtime, and `flake.nix` offers a reproducible dev shell for Nix users.

## Build, Test, and Development Commands
- `cargo build` – compiles the CLI; add `--release` when benchmarking replay fidelity.
- `cargo run -- record result/print_time.wasm -t traces/time.json -- --flag` – records clock calls while forwarding trailing args to the component.
- `cargo run -- replay …` – replays a previously captured trace; ensure the `.wasm` binary matches the trace origin.
- `nix build .` – compiles the example Wasm components and the CLI dependencies into `result/`.
- `cargo test` – runs unit and doc tests; use `cargo test -- --nocapture` when you need stdout.

## Coding Style & Naming Conventions
Use Rust 2021 idioms with 4-space indentation; keep modules `snake_case`, types `CamelCase`, and constants `SCREAMING_SNAKE_CASE`. Run `cargo fmt` before submitting and prefer idiomatic error propagation via `anyhow::Context`. `cargo clippy --all-targets --all-features` is encouraged; heed lint suggestions unless there is a compelling reason not to. Trace event variants should remain descriptive but concise while preserving existing serde tags.

## Testing Guidelines
Favor unit tests alongside the modules they cover; add integration tests under `tests/` when exercising the full record/replay loop. Keep traces committed for deterministic tests small and scrubbed of environment secrets. When fixing bugs, reproduce them with a focused Wasm example under `examples/` and record the expected trace. Manual validation should include a fresh `cargo run -- record` followed by `cargo run -- replay` to confirm parity.

## Commit & Pull Request Guidelines
Commits must follow Conventional Commits (https://www.conventionalcommits.org/en/v1.0.0/), e.g., `feat: add deterministic playback cache` or `fix(playback): guard empty trace`. Use semantic types (`feat`, `fix`, `refactor`, `chore`, `docs`, `test`) and scope when it adds clarity. Keep bodies concise and explain the rationale, not the diff. Group related changes so each commit compiles and passes tests. PRs must cite the reproduced issue, summarize validation steps (commands plus outcomes), and attach any new traces or screenshots that illustrate behavior changes. Link issues where relevant and note any backward-compatibility concerns or follow-up work in the description.
