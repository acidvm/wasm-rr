#!/usr/bin/env bash
set -euo pipefail

# Generate CLI reference documentation using clap-markdown
# This script should be run before building the mdBook documentation

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUTPUT_FILE="${SCRIPT_DIR}/src/cli-reference.md"

# Generate markdown using the gen-cli-docs binary
cargo run --bin gen-cli-docs > "${OUTPUT_FILE}"

echo "Generated CLI documentation at ${OUTPUT_FILE}"
