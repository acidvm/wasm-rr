# Compilation function for C WASM programs using Zig as the compiler
{ pkgs, wasi-adapter }:

{ name, src, sourceFile ? "hello.c" }:
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

    # Compile C to WASI p1 module using Zig
    zig cc \
      -target wasm32-wasi \
      -O2 \
      -o ${name}_module.wasm \
      ${sourceFile}

    # Check the module was created
    ls -la ${name}_module.wasm

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