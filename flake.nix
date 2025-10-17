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
  };

  outputs = {
    nixpkgs,
    flake-utils,
    crane,
    rust-overlay,
    advisory-db,
    ghc-wasm-meta,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [(import rust-overlay)];
        };
        lib = pkgs.lib;

        rustWithWasmTarget = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
          targets = ["wasm32-wasip2"];
        };
        craneLib = (crane.mkLib pkgs).overrideToolchain rustWithWasmTarget;

        examplePackages = example: let
          src = craneLib.cleanCargoSource (craneLib.path ./examples/${example});
        in {
          "${example}-wasm" = craneLib.buildPackage {
            name = "${example}-wasm";
            inherit src;
            inheritToolchain = false;

            CARGO_BUILD_TARGET = "wasm32-wasip2";
            doCheck = false;

            installPhase = ''
              mkdir -p $out
              cp -v target/wasm32-wasip2/release/${example}.wasm $out/
            '';
          };
        };

        examples = ["print_time" "print_args" "print_random" "fetch_quote" "bench_num"];

        # Fetch Javy binary from GitHub releases
        javy = pkgs.stdenv.mkDerivation {
          name = "javy";
          version = "3.1.1";

          src = pkgs.fetchurl {
            url = "https://github.com/bytecodealliance/javy/releases/download/v3.1.1/javy-${
              if pkgs.stdenv.isDarwin && pkgs.stdenv.isAarch64 then "arm-macos"
              else if pkgs.stdenv.isDarwin then "x86_64-macos"
              else if pkgs.stdenv.isLinux && pkgs.stdenv.isAarch64 then "arm-linux"
              else "x86_64-linux"
            }-v3.1.1.gz";
            sha256 = if pkgs.stdenv.isDarwin && pkgs.stdenv.isAarch64 then "sha256-XAS45+Av/EhTH0d1LSH2f/hRyXgb8jx2aCIyTWPSHPQ="
              else if pkgs.stdenv.isDarwin then "sha256-5TIlnxPrN7fPZECpP6Rf9SxJWvNKV8b8NXSc3EpUTzY="
              else if pkgs.stdenv.isLinux && pkgs.stdenv.isAarch64 then "sha256-XxkYdmDLV6T7KvZ1PZ6nWKZBCPLnj6qVZ7vKZZJQqZg="
              else "sha256-NZijbnWU53P12Nh47NpFn70wtowB5aors9vV04/NErY=";
          };

          nativeBuildInputs = [ pkgs.gzip ] ++ lib.optionals pkgs.stdenv.isLinux [
            pkgs.autoPatchelfHook
          ];

          buildInputs = lib.optionals pkgs.stdenv.isLinux [
            pkgs.stdenv.cc.cc.lib
          ];

          unpackPhase = ''
            gunzip -c $src > javy
          '';

          installPhase = ''
            mkdir -p $out/bin
            cp javy $out/bin/javy
            chmod +x $out/bin/javy
          '';

          meta = {
            description = "JavaScript to WebAssembly toolchain";
            homepage = "https://github.com/bytecodealliance/javy";
            platforms = pkgs.lib.platforms.unix;
          };
        };

        # Build JavaScript example using Javy
        js_wordstats-wasm = pkgs.stdenv.mkDerivation {
          name = "js_wordstats-wasm";
          src = ./examples/js_wordstats;

          nativeBuildInputs = with pkgs; [
            javy
            wasm-tools
          ];

          buildPhase = ''
            # Set up HOME directory
            export HOME=$TMPDIR

            # Compile JavaScript to WASI p1 module using Javy
            javy compile index.js -o js_wordstats_module.wasm

            # Convert to WASI p2 component using adapter
            wasm-tools component new js_wordstats_module.wasm \
              --adapt wasi_snapshot_preview1=${wasi-adapter} \
              -o js_wordstats.wasm
          '';

          installPhase = ''
            mkdir -p $out
            cp js_wordstats.wasm $out/
          '';
        };

        packagesForExamples =
          builtins.foldl' (acc: example: acc // examplePackages example) {}
          examples;

        # Fetch the WASI adapter for converting wasip1 to wasip2
        wasi-adapter = pkgs.fetchurl {
          url = "https://github.com/bytecodealliance/wasmtime/releases/download/v37.0.2/wasi_snapshot_preview1.command.wasm";
          sha256 = "1lazj423za0816xi2sb7lvznrp7is3lkv4pil6nf77yj2v3qjvab";
        };

        # Build C Hello World example
        c_hello_world-wasm = pkgs.stdenv.mkDerivation {
          name = "c_hello_world-wasm";
          src = ./examples/c_hello_world;

          nativeBuildInputs = with pkgs; [
            zig
            wasm-tools
          ];

          buildPhase = ''
            # Set up Zig cache directory
            export ZIG_GLOBAL_CACHE_DIR=$TMPDIR/zig-cache
            mkdir -p $ZIG_GLOBAL_CACHE_DIR

            # Compile C to WASI p1 module using Zig
            zig cc \
              -target wasm32-wasi \
              -O2 \
              -o hello_module.wasm \
              hello.c

            # Check the module was created
            ls -la hello_module.wasm

            # Convert to WASI p2 component using adapter
            wasm-tools component new hello_module.wasm \
              --adapt wasi_snapshot_preview1=${wasi-adapter} \
              -o c_hello_world.wasm
          '';

          installPhase = ''
            mkdir -p $out
            cp c_hello_world.wasm $out/
          '';
        };

        # Build Go Hello World example
        go_hello_world-wasm = pkgs.stdenv.mkDerivation {
          name = "go_hello_world-wasm";
          src = ./examples/go_hello_world;

          nativeBuildInputs = with pkgs; [
            tinygo
            wasm-tools
          ];

          buildPhase = ''
            # Set up HOME directory for TinyGo
            export HOME=$TMPDIR

            # Compile Go to WASIp2 component using TinyGo
            tinygo build -target=wasip2 -o go_hello_world.wasm main.go
          '';

          installPhase = ''
            mkdir -p $out
            cp go_hello_world.wasm $out/
          '';
        };

        # Build Haskell Hello World example
        hello_haskell-wasm = pkgs.stdenv.mkDerivation {
          name = "hello_haskell-wasm";
          src = ./examples/hello_haskell;

          nativeBuildInputs = with pkgs; [
            wasm-tools
          ] ++ lib.optionals (ghc-wasm-meta ? packages.${system}) [
            ghc-wasm-meta.packages.${system}.all_9_10
          ];

          buildPhase = ''
            # Set up HOME directory
            export HOME=$TMPDIR

            # Compile directly with GHC (bypassing cabal to avoid network access)
            wasm32-wasi-ghc \
              -o hello-haskell.wasm \
              -O2 \
              Main.hs

            # Convert to WASI p2 component using adapter
            wasm-tools component new hello-haskell.wasm \
              --adapt wasi_snapshot_preview1=${wasi-adapter} \
              -o hello_haskell.wasm
          '';

          installPhase = ''
            mkdir -p $out
            cp hello_haskell.wasm $out/
          '';
        };

        # Fetch pre-built Python WASM runtime
        python-wasm-module = pkgs.fetchurl {
          url = "https://github.com/vmware-labs/webassembly-language-runtimes/releases/download/python%2F3.12.0%2B20231211-040d5a6/python-3.12.0.wasm";
          sha256 = "sha256-5dxaOYsHtU6o/bUDv2j7WD1TPxDsP5MJY+ArlQX3p2M=";
        };

        # Build Python Hello World example
        hello_python-wasm = pkgs.stdenv.mkDerivation {
          name = "hello_python-wasm";
          src = ./examples/hello_python;

          nativeBuildInputs = with pkgs; [
            wasm-tools
          ];

          buildPhase = ''
            # The python-3.12.0.wasm is a WASI p1 module, convert to p2 component
            wasm-tools component new ${python-wasm-module} \
              --adapt wasi_snapshot_preview1=${wasi-adapter} \
              -o hello_python.wasm
          '';

          installPhase = ''
            mkdir -p $out
            cp hello_python.wasm $out/
            # Also copy the Python script for reference
            cp app.py $out/
          '';
        };

        # Build Zig FizzBuzz example
        fizzbuzz_zig-wasm = pkgs.stdenv.mkDerivation {
          name = "fizzbuzz_zig-wasm";
          src = ./examples/fizzbuzz_zig;

          nativeBuildInputs = with pkgs; [
            zig
            wasm-tools
          ];

          buildPhase = ''
            # Set up Zig cache directory
            export ZIG_GLOBAL_CACHE_DIR=$TMPDIR/zig-cache
            mkdir -p $ZIG_GLOBAL_CACHE_DIR

            # Compile Zig to WASI p1 module
            zig build-exe \
              -target wasm32-wasi \
              -O ReleaseSmall \
              -fno-entry \
              -rdynamic \
              main.zig

            # Convert to WASI p2 component using adapter
            wasm-tools component new main.wasm \
              --adapt wasi_snapshot_preview1=${wasi-adapter} \
              -o fizzbuzz_zig.wasm
          '';

          installPhase = ''
            mkdir -p $out
            cp fizzbuzz_zig.wasm $out/
          '';
        };

        # Build counts tool from GitHub release
        counts-wasm = let
          countsSrc = pkgs.fetchFromGitHub {
            owner = "nnethercote";
            repo = "counts";
            rev = "1.0.6";
            sha256 = "sha256-9f+8PBovI6ycsEITWMJ7XXdEe8wtcEBxcB2Cl9RMSr0=";
          };
        in craneLib.buildPackage {
          name = "counts-wasm";
          src = countsSrc;

          CARGO_BUILD_TARGET = "wasm32-wasip2";
          doCheck = false;

          installPhase = ''
            mkdir -p $out
            cp target/wasm32-wasip2/release/counts.wasm $out/
          '';
        };

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

        devShells.default = pkgs.mkShell {
          inputsFrom = [ cargoArtifacts ];
          nativeBuildInputs = with pkgs; [
            # Rust toolchain
            rustWithWasmTarget

            # Documentation tools
            mdbook
            mdbook-linkcheck

            # Development tools
            cargo-watch
            rust-analyzer
          ];

          shellHook = ''
            echo "wasm-rr development environment"
            echo "- cargo build: Build the project"
            echo "- cargo test: Run tests"
            echo "- cd docs && mdbook serve: Serve documentation locally"
            echo "- ./docs/build-cli-docs.sh: Regenerate CLI reference"
          '';
        };
      }
    );
}
