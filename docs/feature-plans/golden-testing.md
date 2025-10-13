# Golden Testing Plan

- Summary: Stand up a reusable golden-testing workflow that keeps replay output locked to recorded traces and flags regressions quickly.
- Owner: Codex assistant (initial draft), wasm-rr maintainers
- Last updated: 2025-10-12

## Description

This initiative introduces an end-to-end golden testing loop for `wasm-rr` replay. We will standardize fixture layout under `golden/<component>/` (e.g., `trace.json`, `stdout.txt`, `stderr.txt`, and optional `metadata.toml`), add helper tooling to capture new fixtures, and wire them into an automated verifier. The suite will begin with coverage for the `print_args` and `print_time` example components, using deterministic inputs that exercise stdout, stderr, environment, and clock behavior. Verification runs via a Nix app (`nix run .#golden-test`) that replays each trace with Nix-provided WASM artifacts and diffs outputs against the committed goldens. Unified diff reporting will surface mismatches, and contributor docs will describe how to refresh fixtures and extend coverage. Longer term, we will scale the suite to additional components and integrate optional timing metrics to observe replay drift.

## Success Criteria

- [x] `nix run .#golden-test` exercises the golden suite (or fails fast) in CI and locally without extra flags.
- [x] Golden fixtures for `print_args` and `print_time` replay consistently across macOS/Linux with identical stdout/stderr checksums.
- [x] A documented helper (`nix run .#golden-fixture`) records traces and writes fixtures in one step using committed scripts.
- [x] Test failures emit a unified diff (trace ID, expected vs. actual) that pinpoints mismatched lines within 5 lines of context.
- [x] Maintaining or adding fixtures (record + validate) takes ≤ 2 manual commands as verified in dry-run walkthrough.

## Resources

- https://martinfowler.com/bliki/SelfTestingCode.html — golden testing patterns
- https://docs.rs/assert_cmd/latest/assert_cmd/ — Rust CLI assertion utilities
- https://docs.rs/predicates/latest/predicates/ — Text comparison helpers
- https://doc.rust-lang.org/cargo/commands/cargo-test.html — Custom test harness integration
- Repository examples in `examples/print_args` and `examples/print_time`
- Existing trace handling in `src/recorder.rs` and `src/playback.rs`

## Dependencies & Integration

- Deterministic stdout/stderr capture from `src/playback.rs`; may require exposing helpers or using Wasmtime hooks.
- Access to stable `.wasm` artifacts (`result/*.wasm`) recorded with the pinned toolchain in `rust-toolchain.toml`.
- Potential new dev-dependencies (`assert_cmd`, `predicates`, `similar`) must remain compatible with current MSRV/CI image.
- CI config (GitHub Actions/Nix) must invoke `nix run .#golden-test`.
- Documentation updates in `CONTRIBUTING.md` and/or `README.md` to teach the workflow.

## Telemetry & Evaluation

- Emit optional verbose output (e.g., extend the Nix app with timing flags) with replay duration and unified diff to aid postmortems.
- Capture basic timing metrics (median replay duration per fixture) during CI and store in build logs; investigate regression thresholds after baseline is collected.
- Record a tally of flaky runs (e.g., by grepping CI artifacts) to identify nondeterministic fixtures and feed back into normalization work.

## Risks & Mitigations

- Risk: Nondeterministic fields (timestamps, ordering) produce flaky diffs.  
  Mitigation: Inject deterministic clocks via replay options or post-process outputs before writing goldens; prefer deterministic inputs in example components.
- Risk: Fixture updates are cumbersome, discouraging adoption.  
  Mitigation: Ship a single helper command that records and diffs fixtures; document the exact workflow in CONTRIBUTING with copy/pastable commands.
- Risk: Golden assets inflate repository size over time.  
  Mitigation: Keep traces minimal (target ≤ 25 KB each), enforce size checks in the helper, and prune obsolete fixtures during reviews.
- Risk: Additional crates slow builds or break MSRV.  
  Mitigation: Evaluate new dev-dependencies for compile time and MSRV compatibility; fall back to in-tree utilities if needed.
- Risk: Platform-specific newline handling causes spurious diffs.  
  Mitigation: Normalize line endings in the helper tooling and assert LF format when writing fixtures.

## Checklist

- [x] Finalize `golden/<component>/` schema and metadata requirements (Owner: Codex assistant, Target date: 2025-10-15)
- [x] Automate trace capture helper (`nix run .#golden-fixture`) (Owner: Codex assistant, Target date: 2025-10-18)
- [x] Record deterministic fixtures for `print_args` and `print_time` (Owner: Codex assistant, Target date: 2025-10-18)
- [x] Implement `nix run .#golden-test` verifier covering replay vs. fixtures with unified diff output (Owner: Codex assistant, Target date: 2025-10-20)
- [x] Integrate golden suite into CI via `nix run .#golden-test` (Owner: Codex assistant, Target date: 2025-10-21)
- [x] Document contributor workflow for maintaining goldens (Owner: Codex assistant, Target date: 2025-10-21)
- [x] Baseline timing metrics and add optional verbose diagnostics (Owner: Codex assistant, Target date: 2025-10-23)

## Open Questions

- Should fixture metadata track toolchain + Wasmtime versions to detect incompatible replays?
- Do we want to support binary diffing for large stdout payloads, or is text diff sufficient?
- How should we name fixtures when multiple traces exist per component (e.g., `print_time/timezones` vs. `print_time/utc`)?
- Can we reuse trace normalization logic for both recording and replay (shared crate vs. helper script)?

## Decision Log

- 2024-05-29 — Created initial plan outlining scope, risks, and tasks.
- 2025-10-12 — Expanded plan with detailed success criteria, schedule, and telemetry strategy.
- 2025-10-12 — Delivered golden testing suite, helper tooling, and documentation; all checklist items complete.
