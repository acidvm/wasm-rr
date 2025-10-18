# Compilation function for Zig WASM programs
{ pkgs, wasi-adapter }:

{ name, src, mainFile ? "main.zig" }:
pkgs.stdenv.mkDerivation {
  name = "${name}-wasm";
  inherit src;

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
      ${mainFile}

    # Convert to WASI p2 component using adapter
    wasm-tools component new ${builtins.replaceStrings [".zig"] [".wasm"] mainFile} \
      --adapt wasi_snapshot_preview1=${wasi-adapter} \
      -o ${name}.wasm
  '';

  installPhase = ''
    mkdir -p $out
    cp ${name}.wasm $out/
  '';
}