{ pkgs
, lib
, craneLib
, allWasmComponents
, wasmEnvVars
, goldenTestScript
}:

let
  # Get Rust toolchain with llvm-tools-preview
  rustToolchain = (pkgs.rust-bin.fromRustupToolchainFile ../rust-toolchain.toml).override {
    extensions = [ "llvm-tools-preview" ];
  };

  # Extract package metadata
  cargoToml = craneLib.crateNameFromCargoToml {
    cargoToml = ../Cargo.toml;
  };

  # Common args for all builds
  commonArgs = {
    src = craneLib.path ../.;
    strictDeps = true;
    RUSTFLAGS = "-Cinstrument-coverage";
  };

  # Build dependencies with coverage instrumentation
  cargoArtifactsCoverage = craneLib.buildDepsOnly (commonArgs // {
    pname = "${cargoToml.pname}-coverage-deps";
  });

  # Build wasm-rr with coverage instrumentation
  wasm-rr-coverage = craneLib.buildPackage (commonArgs // {
    inherit (cargoToml) version;
    pname = "${cargoToml.pname}-coverage";
    cargoArtifacts = cargoArtifactsCoverage;
    doCheck = false;
  });

  # Build test executables with coverage instrumentation
  # We use cargoBuild to build tests separately
  wasm-rr-tests = craneLib.mkCargoDerivation (commonArgs // {
    inherit (cargoToml) version;
    pname = "${cargoToml.pname}-tests";
    cargoArtifacts = cargoArtifactsCoverage;

    buildPhaseCargoCommand = "cargo test --no-run --release --message-format json-render-diagnostics";

    nativeBuildInputs = [ ];

    installPhase = ''
      mkdir -p $out/bin

      # Copy all test executables from the target directory
      if [ -d target/release/deps ]; then
        find target/release/deps -maxdepth 1 -type f -executable | while read test_bin; do
          # Filter out libraries and other non-test files
          if [[ "$test_bin" != *.so ]] && [[ "$test_bin" != *.dylib ]] && [[ "$test_bin" != *.d ]]; then
            # Check if it's a test binary by looking at the filename
            basename=$(basename "$test_bin")
            if [[ "$basename" == wasm_rr-* ]] || [[ "$basename" == *-* ]]; then
              cp "$test_bin" "$out/bin/" || true
            fi
          fi
        done
      fi

      # Verify we got test binaries
      if [ -z "$(ls -A $out/bin 2>/dev/null)" ]; then
        echo "Warning: No test binaries found, creating empty directory"
      fi
    '';
  });
in
pkgs.stdenv.mkDerivation {
  name = "wasm-rr-coverage-report";

  nativeBuildInputs = [
    wasm-rr-coverage
    wasm-rr-tests
    rustToolchain
    pkgs.python3
    pkgs.diffutils
    pkgs.findutils
    pkgs.coreutils
  ];

  # We don't need sources in the build, just the binaries
  dontUnpack = true;

  buildPhase = ''
    set -x

    # Add llvm-tools to PATH
    # Find the llvm-tools directory in the Rust toolchain
    for dir in ${rustToolchain}/lib/rustlib/*/bin; do
      if [ -d "$dir" ]; then
        export PATH="$dir:$PATH"
        echo "Added to PATH: $dir"
        ls -la "$dir" || true
        break
      fi
    done

    # Verify llvm-profdata is available
    which llvm-profdata || echo "Warning: llvm-profdata not found in PATH"
    which llvm-cov || echo "Warning: llvm-cov not found in PATH"

    # Create working directory
    mkdir -p $TMPDIR/coverage
    cd $TMPDIR/coverage

    # Set up profraw collection
    export LLVM_PROFILE_FILE="$TMPDIR/coverage/wasm-rr-%p-%m.profraw"

    # Copy golden fixtures to writable location
    cp -r ${../golden} ./golden
    chmod -R u+w ./golden

    # Set up environment for golden tests
    export WASM_RR_BIN="${wasm-rr-coverage}/bin/wasm-rr"
    ${wasmEnvVars}

    echo "=== Running golden tests with coverage ==="
    # Write golden test script to file and execute it
    cat > $TMPDIR/run-golden-tests.sh <<'GOLDEN_EOF'
${goldenTestScript}
GOLDEN_EOF
    chmod +x $TMPDIR/run-golden-tests.sh

    # Run golden tests (this will generate .profraw files)
    bash $TMPDIR/run-golden-tests.sh || echo "Golden tests completed with failures (expected for some tests)"

    echo "=== Running cargo tests with coverage ==="
    # Run all test binaries
    if [ -d "${wasm-rr-tests}/bin" ] && [ -n "$(ls -A ${wasm-rr-tests}/bin 2>/dev/null)" ]; then
      for test_bin in ${wasm-rr-tests}/bin/*; do
        if [ -x "$test_bin" ]; then
          echo "Running test: $(basename $test_bin)"
          "$test_bin" --test-threads=1 || true  # Don't fail if tests fail
        fi
      done
    else
      echo "No test binaries found to run"
    fi

    echo "=== Collecting profraw files ==="
    # List all generated profraw files
    profraw_count=$(find $TMPDIR/coverage -name "*.profraw" -type f | wc -l)
    echo "Found $profraw_count profraw files"

    if [ "$profraw_count" -eq 0 ]; then
      echo "ERROR: No profraw files generated!"
      exit 1
    fi

    # Merge all profraw files into a single profdata file
    echo "=== Merging profraw files ==="
    llvm-profdata merge -sparse \
      $TMPDIR/coverage/*.profraw \
      -o $TMPDIR/coverage/merged.profdata

    echo "=== Generating coverage reports ==="

    # Collect all binaries for coverage analysis
    BINARIES=""
    BINARIES="$BINARIES -object ${wasm-rr-coverage}/bin/wasm-rr"

    # Add test binaries if they exist
    if [ -d "${wasm-rr-tests}/bin" ] && [ -n "$(ls -A ${wasm-rr-tests}/bin 2>/dev/null)" ]; then
      for test_bin in ${wasm-rr-tests}/bin/*; do
        if [ -x "$test_bin" ]; then
          BINARIES="$BINARIES -object $test_bin"
        fi
      done
    fi

    echo "Coverage binaries: $BINARIES"

    # Generate lcov.info
    llvm-cov export \
      $BINARIES \
      --instr-profile=$TMPDIR/coverage/merged.profdata \
      --format=lcov \
      --ignore-filename-regex='/.cargo/' \
      --ignore-filename-regex='/rustc/' \
      --ignore-filename-regex='/nix/store/' \
      > $TMPDIR/coverage/lcov.info

    # Generate HTML report
    llvm-cov show \
      $BINARIES \
      --instr-profile=$TMPDIR/coverage/merged.profdata \
      --format=html \
      --output-dir=$TMPDIR/coverage/html \
      --ignore-filename-regex='/.cargo/' \
      --ignore-filename-regex='/rustc/' \
      --ignore-filename-regex='/nix/store/' \
      --show-line-counts-or-regions \
      --show-instantiations

    # Generate text summary
    llvm-cov report \
      $BINARIES \
      --instr-profile=$TMPDIR/coverage/merged.profdata \
      --ignore-filename-regex='/.cargo/' \
      --ignore-filename-regex='/rustc/' \
      --ignore-filename-regex='/nix/store/' \
      > $TMPDIR/coverage/summary.txt

    echo "=== Coverage summary ==="
    cat $TMPDIR/coverage/summary.txt
  '';

  installPhase = ''
    mkdir -p $out

    # Copy all coverage artifacts
    cp $TMPDIR/coverage/lcov.info $out/
    cp $TMPDIR/coverage/summary.txt $out/
    cp -r $TMPDIR/coverage/html $out/

    # Copy the profdata file as well for potential reuse
    cp $TMPDIR/coverage/merged.profdata $out/

    echo ""
    echo "Coverage report generated successfully!"
    echo ""
    echo "Outputs:"
    echo "  - lcov.info:    $out/lcov.info"
    echo "  - HTML report:  $out/html/index.html"
    echo "  - Text summary: $out/summary.txt"
    echo "  - Profdata:     $out/merged.profdata"
    echo ""
  '';
}
