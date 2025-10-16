#!/usr/bin/env bash
set -euo pipefail

: "${WASM_RR_BIN:?WASM_RR_BIN must be set}"
: "${PRINT_ARGS_WASM:?PRINT_ARGS_WASM must be set}"
: "${PRINT_TIME_WASM:?PRINT_TIME_WASM must be set}"
: "${PRINT_RANDOM_WASM:?PRINT_RANDOM_WASM must be set}"
: "${FETCH_QUOTE_WASM:?FETCH_QUOTE_WASM must be set}"
: "${BENCH_NUM_WASM:?BENCH_NUM_WASM must be set}"
: "${C_HELLO_WORLD_WASM:?C_HELLO_WORLD_WASM must be set}"
: "${GO_HELLO_WORLD_WASM:?GO_HELLO_WORLD_WASM must be set}"
: "${HELLO_HASKELL_WASM:?HELLO_HASKELL_WASM must be set}"
: "${JS_WORDSTATS_WASM:?JS_WORDSTATS_WASM must be set}"
: "${COUNTS_WASM:?COUNTS_WASM must be set}"

resolve_wasm() {
  case "$1" in
    print_args) printf '%s\n' "$PRINT_ARGS_WASM" ;;
    print_time) printf '%s\n' "$PRINT_TIME_WASM" ;;
    print_random) printf '%s\n' "$PRINT_RANDOM_WASM" ;;
    fetch_quote) printf '%s\n' "$FETCH_QUOTE_WASM" ;;
    bench_num) printf '%s\n' "$BENCH_NUM_WASM" ;;
    c_hello_world) printf '%s\n' "$C_HELLO_WORLD_WASM" ;;
    go_hello_world) printf '%s\n' "$GO_HELLO_WORLD_WASM" ;;
    hello_haskell) printf '%s\n' "$HELLO_HASKELL_WASM" ;;
    js_wordstats) printf '%s\n' "$JS_WORDSTATS_WASM" ;;
    counts) printf '%s\n' "$COUNTS_WASM" ;;
    *)
      echo "unknown component: $1" >&2
      return 1
      ;;
  esac
}

failures=0

while IFS= read -r metadata; do
  dir="$(dirname "$metadata")"
  mapfile -t meta < <(python3 - <<'PY' "$metadata"
import sys
import tomllib

with open(sys.argv[1], 'rb') as fh:
    data = tomllib.load(fh)

print(data["component"])
print(data.get("scenario", ""))
print(data["trace"])
print(data["stdout"])
print(data["stderr"])
print("true" if data.get("must_fail", False) else "false")
PY
  )

  component="${meta[0]}"
  scenario="${meta[1]}"
  trace_rel="${meta[2]}"
  stdout_rel="${meta[3]}"
  stderr_rel="${meta[4]}"
  must_fail="${meta[5]}"

  wasm_path="$(resolve_wasm "$component")" || {
    failures=$((failures + 1))
    continue
  }

  trace_file="$dir/$trace_rel"
  stdout_file="$dir/$stdout_rel"
  stderr_file="$dir/$stderr_rel"

  if [[ ! -f "$trace_file" ]]; then
    echo "missing trace file: $trace_file" >&2
    failures=$((failures + 1))
    continue
  fi

  label="$component"
  if [[ -n "$scenario" ]]; then
    label="$label/$scenario"
  fi

  actual_stdout="$(mktemp)"
  actual_stderr="$(mktemp)"
  fixture_fail=0

  if ! "$WASM_RR_BIN" replay "$wasm_path" "$trace_file" >"$actual_stdout" 2>"$actual_stderr"; then
    echo "replay failed for $label" >&2
    cat "$actual_stderr" >&2 || true
    fixture_fail=1
  else
    if ! diff -u "$stdout_file" "$actual_stdout"; then
      echo "stdout mismatch for $label" >&2
      fixture_fail=1
    fi
    if ! diff -u "$stderr_file" "$actual_stderr"; then
      echo "stderr mismatch for $label" >&2
      fixture_fail=1
    fi
  fi

  # Handle must_fail tests
  if [[ "$must_fail" == "true" ]]; then
    if [[ "$fixture_fail" -eq 1 ]]; then
      echo "✓ Expected failure: $label"
    else
      echo "✗ Unexpected pass: $label (marked as must_fail but succeeded)" >&2
      failures=$((failures + 1))
    fi
  else
    if [[ "$fixture_fail" -eq 0 ]]; then
      echo "✔ $label"
    else
      failures=$((failures + 1))
    fi
  fi

  rm -f "$actual_stdout" "$actual_stderr"
done < <(find golden -name metadata.toml | sort)

if [[ "$failures" -ne 0 ]]; then
  exit "$failures"
fi
