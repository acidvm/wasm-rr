{
  description = "Build all WASM examples using crane";

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

        examples = ["print_time" "print_args"];

        packagesForExamples =
          builtins.foldl' (acc: example: acc // examplePackages example) {}
          examples;
      in {
        packages =
          packagesForExamples
          // {
            default = pkgs.linkFarmFromDrvs "wasm-examples" (map (
                example: self.packages.${system}."${example}-wasm"
              )
              examples);
          };
      }
    );
}
