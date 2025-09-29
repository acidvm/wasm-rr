{
  description = "Build print_time example to wasm32-wasi using crane";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    crane,
    rust-overlay,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [(import rust-overlay)];
        };

        rustWithWasmTarget = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
          targets = ["wasm32-wasip2"];
        };
        craneLib = (crane.mkLib pkgs).overrideToolchain rustWithWasmTarget;

        src = craneLib.cleanCargoSource (craneLib.path ./examples/print_time);
      in rec {
        # Optionally build deps first for caching
        cargoArtifacts = craneLib.buildDepsOnly {
          inherit src;
          CARGO_BUILD_TARGET = "wasm32-wasip2";
        };

        packages.print_time-wasm = craneLib.buildPackage {
          pname = "print_time-wasm";
          version = "0.1.0";
          inherit src cargoArtifacts;
          inheritToolchain = false;

          # Build the example as wasm32-wasi
          CARGO_BUILD_TARGET = "wasm32-wasip2";
          doCheck = false;

          # Install the produced .wasm artifact
          installPhase = ''
            mkdir -p $out
            cp -v target/wasm32-wasip2/release/print_time.wasm $out/
          '';
        };

        packages.default = self.packages.${system}.print_time-wasm;
      }
    );
}
