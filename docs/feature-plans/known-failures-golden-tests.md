# Known Failures in Golden Tests Plan

- Summary: Extend golden testing framework to support expected failures for non-deterministic components like print_random.
- Owner: wasm-rr maintainers
- Last updated: 2025-10-13

## Description

This initiative extends the golden testing infrastructure to support components that are expected to fail replay verification due to non-deterministic behavior. The primary motivation is to enable testing of the `print_random` example, which uses `wasi:random/random` to generate random numbers that are not yet captured in execution traces. By introducing a `must_fail` flag in golden test metadata, we can distinguish between expected failures (components we know will fail) and unexpected regressions (components that should pass but don't).

The system will track these known failures in CI while preventing them from masking real regressions. When random value recording is eventually implemented, these tests can be converted from must_fail to regular passing tests by simply updating their metadata.

## Success Criteria

- [x] Golden tests with `must_fail = true` are executed but expected to fail
- [x] Test runner reports "✓ Expected failure" for must_fail tests that fail
- [x] Test runner reports "✗ Unexpected pass" if a must_fail test unexpectedly passes
- [x] `print_random` golden fixtures exist and are marked as must_fail
- [x] CI continues to pass with known failures present (verified via `nix run .#golden-test`)
- [x] Clear distinction in output between expected failures and actual failures

## Resources

- Existing golden test implementation in `nix/golden-test.sh`
- Golden test documentation in `docs/feature-plans/golden-testing.md`
- `print_random` example in `examples/print_random/`
- wasi:random/random implementation in `src/main.rs:464`
- Golden fixture structure in `golden/` directory

## Dependencies & Integration

- Golden test runner script (`nix/golden-test.sh`) must be updated to parse must_fail flag
- Metadata schema for `metadata.toml` needs extension with optional must_fail field
- `flake.nix` must include print_random in the examples list and WASM build
- CI workflows should continue to function with known failures present
- Future trace recording enhancements for random values will convert these to passing tests

### Implementation Notes

- The print_random example already existed in `examples/print_random/` with proper Cargo.toml
- The WASM artifact was already being built by the nix flake (found in `result/print_random.wasm`)
- Only needed to add environment variable export and resolve_wasm case in test scripts

## Implementation Approach

1. **Metadata Schema Extension**:

   - Add optional `must_fail = true/false` field to metadata.toml
   - Default to false when not specified for backward compatibility

2. **Test Runner Updates**:

   - Parse must_fail flag from metadata
   - For must_fail tests:
     - If replay fails or outputs mismatch: Report as "✓ Expected failure: <component>"
     - If replay succeeds with matching outputs: Report as "✗ Unexpected pass: <component>" and fail the test
   - Maintain separate counters for expected vs unexpected failures

3. **print_random Fixtures**:
   - Record trace using existing golden-fixture tooling
   - Capture stdout showing random number output
   - Mark as `must_fail = true` in metadata.toml
   - Document that failure is expected until random recording is implemented

## Risks & Mitigations

- Risk: Developers might accidentally mark real bugs as must_fail.
  Mitigation: Require explicit documentation in metadata explaining why failure is expected; review must_fail additions carefully in PRs.

- Risk: must_fail tests might hide actual regressions if they start failing differently.
  Mitigation: Consider capturing and validating the failure reason/pattern, not just the failure itself.

- Risk: Confusion about when to use must_fail vs fixing the underlying issue.
  Mitigation: Clear documentation that must_fail is only for known platform limitations, not test bugs.

## Implementation Steps

- [x] Extend metadata.toml parser in golden-test.sh to handle must_fail field
- [x] Update test runner logic to handle expected failures correctly
- [x] Add print_random to flake.nix examples list (export PRINT_RANDOM_WASM)
- [x] Build print_random WASM component (available in result/)
- [x] Create golden fixtures for print_random with must_fail flag
- [x] Update test output formatting to clearly show expected failures
- [x] Document must_fail usage in AGENTS.md
- [x] Verify CI passes with known failures present

## Open Questions

- Should we capture the expected failure message/pattern to ensure failures happen for the right reason? (Future enhancement)
- ~How should the test summary report expected failures vs actual failures?~ Resolved: Expected failures show "✓ Expected failure", unexpected passes show "✗ Unexpected pass"
- ~Should must_fail tests contribute to the exit code differently than regular failures?~ Resolved: Expected failures don't increment failure count, unexpected passes do
- Do we want a TODO/tracking mechanism for converting must_fail tests to regular tests? (Future consideration)

## Decision Log

- 2025-10-13 — Created initial plan for supporting known failures in golden tests
- 2025-10-13 — Chose must_fail flag approach over separate directory or file naming convention for clarity
- 2025-10-13 — Implemented complete feature with all success criteria met
