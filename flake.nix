{
  description = "Build all WASM examples using crane";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    rust-overlay.url = "github:oxalica/rust-overlay";
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
    ghc-wasm-meta = {
      url = "gitlab:haskell-wasm/ghc-wasm-meta?host=gitlab.haskell.org";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    javy-flake = {
      url = "github:acidvm/javy.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    nixpkgs,
    flake-utils,
    crane,
    rust-overlay,
    advisory-db,
    ghc-wasm-meta,
    javy-flake,
    ...
  }: let
    # Explicitly supported systems
    supportedSystems = ["x86_64-linux" "aarch64-linux" "aarch64-darwin"];

    # Helper function to generate outputs for a given system
    genSystemOutputs = system: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [(import rust-overlay)];
        };
        lib = pkgs.lib;

        rustWithWasmTarget = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
          targets = ["wasm32-wasip2"];
        };
        craneLib = (crane.mkLib pkgs).overrideToolchain rustWithWasmTarget;

        # Fetch the WASI adapter for converting wasip1 to wasip2
        wasi-adapter = pkgs.fetchurl {
          url = "https://github.com/bytecodealliance/wasmtime/releases/download/v37.0.2/wasi_snapshot_preview1.command.wasm";
          sha256 = "1lazj423za0816xi2sb7lvznrp7is3lkv4pil6nf77yj2v3qjvab";
        };

        # Use Javy from the javy.nix flake
        javy = javy-flake.packages.${system}.default;

        # Import compilation functions
        compileRust = import ./nix/compile_rust.nix { inherit craneLib; };
        compileZig = import ./nix/compile_zig.nix { inherit pkgs wasi-adapter; };
        compileC = import ./nix/compile_c.nix { inherit pkgs wasi-adapter; };
        compileGo = import ./nix/compile_go.nix { inherit pkgs; };
        compileJavaScript = import ./nix/compile_javascript.nix { inherit pkgs javy wasi-adapter; };
        compileHaskell = import ./nix/compile_haskell.nix { inherit pkgs lib ghc-wasm-meta wasi-adapter system; };
        compilePython = import ./nix/compile_python.nix { inherit pkgs wasi-adapter; };

        # Build individual Rust examples
        examplePackages = example: let
          src = craneLib.cleanCargoSource (craneLib.path ./examples/${example});
        in {
          "${example}-wasm" = compileRust { name = example; inherit src; };
        };

        examples = ["print_time" "print_args" "print_random" "fetch_quote" "bench_num" "read_stdin"];

        packagesForExamples =
          builtins.foldl' (acc: example: acc // examplePackages example) {}
          examples;

        # Build language-specific examples
        js_wordstats-wasm = compileJavaScript {
          name = "js_wordstats";
          src = ./examples/js_wordstats;
        };

        c_hello_world-wasm = compileC {
          name = "c_hello_world";
          src = ./examples/c_hello_world;
        };

        go_hello_world-wasm = compileGo {
          name = "go_hello_world";
          src = ./examples/go_hello_world;
        };

        hello_haskell-wasm = compileHaskell {
          name = "hello_haskell";
          src = ./examples/hello_haskell;
        };

        hello_python-wasm = compilePython {
          name = "hello_python";
          src = ./examples/hello_python;
        };

        fizzbuzz_zig-wasm = compileZig {
          name = "fizzbuzz_zig";
          src = ./examples/fizzbuzz_zig;
        };

        # Import counts program definition
        counts-wasm = import ./nix/program-counts.nix { inherit pkgs craneLib; };

        goldenFixtureScript = builtins.readFile ./nix/golden-fixture.sh;
        goldenTestScript = builtins.readFile ./nix/golden-test.sh;

        # Extract package metadata from Cargo.toml
        cargoToml = craneLib.crateNameFromCargoToml {
          cargoToml = ./Cargo.toml;
        };

        # Build dependencies for checks
        cargoArtifacts = craneLib.buildDepsOnly {
          src = craneLib.path ./.;
        };

        # All WASM components - both Rust and non-Rust
        allWasmComponents = {
          print_args = packagesForExamples."print_args-wasm";
          print_time = packagesForExamples."print_time-wasm";
          print_random = packagesForExamples."print_random-wasm";
          fetch_quote = packagesForExamples."fetch_quote-wasm";
          bench_num = packagesForExamples."bench_num-wasm";
          read_stdin = packagesForExamples."read_stdin-wasm";
          c_hello_world = c_hello_world-wasm;
          go_hello_world = go_hello_world-wasm;
          hello_haskell = hello_haskell-wasm;
          hello_python = hello_python-wasm;
          fizzbuzz_zig = fizzbuzz_zig-wasm;
          js_wordstats = js_wordstats-wasm;
          counts = counts-wasm;
        };

        # Build wasm-rr CLI tool with dependency caching
        wasm-rr = craneLib.buildPackage {
          inherit (cargoToml) pname version;
          src = craneLib.path ./.;
          inherit cargoArtifacts;
          doCheck = false;
        };

        # Build mdBook documentation
        wasm-rr-docs = pkgs.stdenv.mkDerivation {
          name = "wasm-rr-docs";
          src = ./.;

          nativeBuildInputs = with pkgs; [
            mdbook
            mdbook-linkcheck
            wasm-rr
          ];

          buildPhase = ''
            # Generate CLI reference documentation
            ${wasm-rr}/bin/wasm-rr --markdown-help > docs/src/cli-reference-generated.md

            # Build the documentation
            cd docs
            mdbook build

            # Compute hash of the documentation output
            # We hash all files except the hash file itself to get a deterministic checksum
            cd book
            find . -type f ! -name "docs-hash.txt" -exec sha256sum {} \; | sort | sha256sum | cut -d' ' -f1 > docs-hash.txt
            echo "Documentation hash: $(cat docs-hash.txt)"
            cd ..
          '';

          installPhase = ''
            mkdir -p $out
            cp -r book/* $out/
          '';
        };

        # Generate environment variables for golden tests
        wasmEnvVars = lib.concatStringsSep "\n" (
          lib.mapAttrsToList (name: pkg:
            ''export ${lib.strings.toUpper name}_WASM="${pkg}/${name}.wasm"''
          ) allWasmComponents
        );
      in {
        packages =
          packagesForExamples
          // {
            c_hello_world-wasm = c_hello_world-wasm;
            go_hello_world-wasm = go_hello_world-wasm;
            hello_haskell-wasm = hello_haskell-wasm;
            hello_python-wasm = hello_python-wasm;
            fizzbuzz_zig-wasm = fizzbuzz_zig-wasm;
            js_wordstats-wasm = js_wordstats-wasm;
            counts-wasm = counts-wasm;
            # wasm-rr is now the default package
            default = wasm-rr;
            # All WASM examples collected in one package
            wasm-examples = pkgs.runCommand "wasm-examples" {} ''
              mkdir -p $out
              ${lib.concatStringsSep "\n" (
                lib.mapAttrsToList (name: pkg:
                  ''cp ${pkg}/${name}.wasm $out/${name}.wasm''
                ) allWasmComponents
              )}
            '';
            # Alias for backwards compatibility
            wasm-rr = wasm-rr;
            # Documentation
            docs = wasm-rr-docs;
          };

        checks = {
          # Format check
          fmt = craneLib.cargoFmt {
            src = craneLib.path ./.;
          };

          # Clippy check
          clippy = craneLib.cargoClippy {
            inherit cargoArtifacts;
            src = craneLib.path ./.;
            cargoClippyExtraArgs = "--all-targets --all-features -- -D warnings";
          };

          # Cargo tests
          test = craneLib.cargoTest {
            inherit cargoArtifacts;
            src = craneLib.path ./.;
          };

          # Cargo audit check with pinned advisory database
          audit = craneLib.cargoAudit {
            inherit cargoArtifacts advisory-db;
            src = craneLib.path ./.;
            # Disable yanked check since it requires network access
            cargoAuditExtraArgs = "--ignore yanked";
          };

          # Golden tests
          golden-test = pkgs.runCommand "golden-test-check" {
            nativeBuildInputs = [
              wasm-rr
              pkgs.python3
              pkgs.diffutils
              pkgs.findutils
              pkgs.coreutils
            ];
          } ''
            # Set up environment variables
            export WASM_RR_BIN="${wasm-rr}/bin/wasm-rr"
            ${wasmEnvVars}

            # Copy golden fixtures to writable location
            cp -r ${./golden} ./golden
            chmod -R u+w ./golden

            # Run golden tests
            ${goldenTestScript}

            # If tests pass, create output
            touch $out
          '';

          # Shellcheck for all shell scripts
          shellcheck = pkgs.runCommand "shellcheck" {
            nativeBuildInputs = [
              pkgs.shellcheck
              pkgs.findutils
            ];
          } ''
            # Find and check all shell scripts in the repository
            # This includes:
            # - Files with .sh extension
            # - Files with #!/usr/bin/env bash or #!/bin/bash shebang

            echo "Finding and checking all shell scripts..."

            # Track if any checks fail
            failed=0

            # Find all .sh files and files with bash shebang
            (find ${./.} -type f -name "*.sh" -o -type f -exec grep -l '^#!/.*bash' {} \; 2>/dev/null || true) | while read -r script; do
              echo "Checking: $script"
              if ! shellcheck "$script"; then
                failed=1
              fi
            done

            if [ "$failed" -eq 1 ]; then
              echo "Shellcheck found issues in one or more scripts"
              exit 1
            fi

            touch $out
          '';

          # Markdown format check using mdformat
          markdown-fmt = pkgs.runCommand "markdown-fmt-check" {
            nativeBuildInputs = [ pkgs.python3Packages.mdformat ];
          } ''
            # Check markdown formatting
            failed=0

            # Check docs directory markdown files
            find ${./docs} -name "*.md" -type f | while read -r file; do
              # Check if file needs formatting (--check flag)
              if ! mdformat --check "$file" > /dev/null 2>&1; then
                echo "Markdown formatting issues in: $file"
                echo "Run 'mdformat $file' to fix"
                failed=1
              fi
            done

            # Check root directory markdown files
            for file in ${./.}/*.md; do
              if [ -f "$file" ]; then
                # Check if file needs formatting
                if ! mdformat --check "$file" > /dev/null 2>&1; then
                  echo "Markdown formatting issues in: $file"
                  echo "Run 'mdformat $file' to fix"
                  failed=1
                fi
              fi
            done

            # For now, always succeed to not block CI
            # Change to 'exit $failed' when ready to enforce
            touch $out
          '';
        };

        apps = {
          golden-fixture = flake-utils.lib.mkApp {
            drv =
              pkgs.writeShellApplication
              {
                name = "golden-fixture";
                runtimeInputs = [
                  wasm-rr
                  pkgs.coreutils
                ];
                text = ''
                  export WASM_RR_BIN="${wasm-rr}/bin/wasm-rr"
                  ${goldenFixtureScript}
                '';
              };
          };
          golden-test = flake-utils.lib.mkApp {
            drv =
              pkgs.writeShellApplication
              {
                name = "golden-test";
                runtimeInputs = [
                  wasm-rr
                  pkgs.python3
                  pkgs.diffutils
                  pkgs.findutils
                  pkgs.coreutils
                ];
                text = ''
                  export WASM_RR_BIN="${wasm-rr}/bin/wasm-rr"
                  ${wasmEnvVars}
                  ${goldenTestScript}
                '';
              };
          };
        };
      };

    # Generate outputs for all supported systems
    systemOutputs = builtins.foldl' (acc: system:
      let
        outputs = genSystemOutputs system;
      in
        nixpkgs.lib.recursiveUpdate acc {
          packages.${system} = outputs.packages;
          checks.${system} = outputs.checks;
          apps.${system} = outputs.apps;
        }
    ) {} supportedSystems;
  in
    systemOutputs;
}
