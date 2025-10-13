#!/usr/bin/env bash
set -euo pipefail

: "${WASM_RR_BIN:?WASM_RR_BIN must be set}"
: "${PRINT_ARGS_WASM:?PRINT_ARGS_WASM must be set}"
: "${PRINT_TIME_WASM:?PRINT_TIME_WASM must be set}"

resolve_wasm() {
  case "$1" in
    print_args) printf '%s\n' "$PRINT_ARGS_WASM" ;;
    print_time) printf '%s\n' "$PRINT_TIME_WASM" ;;
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
PY
  )

  component="${meta[0]}"
  scenario="${meta[1]}"
  trace_rel="${meta[2]}"
  stdout_rel="${meta[3]}"
  stderr_rel="${meta[4]}"

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

  if [[ "$fixture_fail" -eq 0 ]]; then
    echo "✔ $label"
  else
    failures=$((failures + 1))
  fi

  rm -f "$actual_stdout" "$actual_stderr"
done < <(find golden -name metadata.toml | sort)

if [[ "$failures" -ne 0 ]]; then
  exit "$failures"
fi
