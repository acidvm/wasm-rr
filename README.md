`wasm-rr` records and replays the execution of WebAssembly programs.

## Golden Testing Workflow

- Build the CLI with `nix build .#wasm-rr` (or the examples with `nix build .`), which produces the binary under `result/bin/wasm-rr`.
- Seed or refresh fixtures (after the component artifacts exist in `result/`) with `nix run .#golden-fixture -- --component print_args -- hello world`. Repeat for other components (e.g., `nix run .#golden-fixture -- --component print_time`).
- Validate recorded goldens with `nix run .#golden-test`, which replays each trace using the Nix-built artifacts and diffs stdout/stderr.
- Use `GOLDEN_DIAG=1 cargo test --test golden` when you need per-fixture timings in CI or during investigations.

Each fixture lives under `golden/<component>/` and includes `trace.json`, `stdout.txt`, `stderr.txt`, and `metadata.toml`. The Nix helper builds the CLI, replays once per fixture, and writes outputs with LF line endings so goldens stay consistent across machines.
