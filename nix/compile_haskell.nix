# Compilation function for Haskell WASM programs
{ pkgs, lib, ghc-wasm-meta, wasi-adapter, system }:

{ name, src, mainFile ? "Main.hs" }:
pkgs.stdenv.mkDerivation {
  name = "${name}-wasm";
  inherit src;

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
      -o ${name}_module.wasm \
      -O2 \
      ${mainFile}

    # Convert to WASI p2 component using adapter
    wasm-tools component new ${name}_module.wasm \
      --adapt wasi_snapshot_preview1=${wasi-adapter} \
      -o ${name}.wasm
  '';

  installPhase = ''
    mkdir -p $out
    cp ${name}.wasm $out/
  '';
}