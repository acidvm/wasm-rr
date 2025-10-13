#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: golden-fixture --component NAME [--wasm PATH] [--scenario NAME] [--] [ARGS...]
EOF
}

component=""
scenario=""
wasm=""
args=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --component)
      component="$2"
      shift 2
      ;;
    --scenario)
      scenario="$2"
      shift 2
      ;;
    --wasm)
      wasm="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    --)
      shift
      args=("$@")
      break
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ ! -f Cargo.toml ]]; then
  echo "golden-fixture must be run from the repository root" >&2
  exit 1
fi

: "${WASM_RR_BIN:?WASM_RR_BIN must be set}"

if [[ -z "$component" ]]; then
  if [[ -n "$wasm" ]]; then
    base="$(basename "$wasm")"
    component="${base%.wasm}"
  else
    echo "missing --component (or --wasm to infer one)" >&2
    usage >&2
    exit 1
  fi
fi

if [[ -z "$wasm" ]]; then
  wasm="result/${component}.wasm"
fi

if [[ ! -f "$wasm" ]]; then
  echo "wasm artifact not found at $wasm" >&2
  exit 1
fi

output_dir="golden/${component}"
if [[ -n "$scenario" ]]; then
  output_dir="${output_dir}/${scenario}"
fi
mkdir -p "$output_dir"

trace="${output_dir}/trace.json"
stdout_file="${output_dir}/stdout.txt"
stderr_file="${output_dir}/stderr.txt"
metadata="${output_dir}/metadata.toml"

cmd=("$WASM_RR_BIN" "record" "$wasm" "-t" "$trace")
if [[ ${#args[@]} -gt 0 ]]; then
  cmd+=("--")
  cmd+=("${args[@]}")
fi
"${cmd[@]}"

cmd=("$WASM_RR_BIN" "replay" "$wasm" "$trace")
"${cmd[@]}" >"$stdout_file" 2>"$stderr_file"

{
  printf 'component = "%s"\n' "$component"
  if [[ -n "$scenario" ]]; then
    printf 'scenario = "%s"\n' "$scenario"
  fi
  printf 'trace = "trace.json"\n'
  printf 'stdout = "stdout.txt"\n'
  printf 'stderr = "stderr.txt"\n'
} >"$metadata"

echo "Updated $output_dir"
