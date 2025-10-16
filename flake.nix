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
    self,
    nixpkgs,
    flake-utils,
    crane,
    rust-overlay,
    advisory-db,
    ghc-wasm-meta,
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
          # Optionally build deps first for caching
          "${example}-artifacts" = craneLib.buildDepsOnly {
            inherit src;
            CARGO_BUILD_TARGET = "wasm32-wasip2";
          };

          "${example}-wasm" = craneLib.buildPackage {
            name = "${example}-wasm";
            inherit src;
            cargoArtifacts = self.packages.${system}."${example}-artifacts";
            inheritToolchain = false;

            CARGO_BUILD_TARGET = "wasm32-wasip2";
            doCheck = false;

            installPhase = ''
              mkdir -p $out
              cp -v target/wasm32-wasip2/release/${example}.wasm $out/
            '';
          };
        };

        examples = ["print_time" "print_args" "print_random" "fetch_quote"];

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
          c_hello_world = c_hello_world-wasm;
          go_hello_world = go_hello_world-wasm;
          hello_haskell = hello_haskell-wasm;
          counts = counts-wasm;
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
            counts-wasm = counts-wasm;
            # wasm-rr is now the default package
            default = craneLib.buildPackage {
              pname = "wasm-rr";
              version = "0.1.0";
              src = craneLib.path ./.;
              doCheck = false;
            };
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
            wasm-rr = self.packages.${system}.default;
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
              self.packages.${system}.default
              pkgs.python3
              pkgs.diffutils
              pkgs.findutils
              pkgs.coreutils
            ];
          } ''
            # Set up environment variables
            export WASM_RR_BIN="${self.packages.${system}.default}/bin/wasm-rr"
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
                  self.packages.${system}.default
                  pkgs.coreutils
                ];
                text = ''
                  export WASM_RR_BIN="${self.packages.${system}.default}/bin/wasm-rr"
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
                  self.packages.${system}.default
                  pkgs.python3
                  pkgs.diffutils
                  pkgs.findutils
                  pkgs.coreutils
                ];
                text = ''
                  export WASM_RR_BIN="${self.packages.${system}.default}/bin/wasm-rr"
                  ${wasmEnvVars}
                  ${goldenTestScript}
                '';
              };
          };
        };
      }
    );
}
