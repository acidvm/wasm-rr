# Compilation function for JavaScript WASM programs using Javy
{ pkgs, javy, wasi-adapter }:

{ name, src, mainFile ? "index.js" }:
pkgs.stdenv.mkDerivation {
  name = "${name}-wasm";
  inherit src;

  nativeBuildInputs = with pkgs; [
    javy
    wasm-tools
  ];

  buildPhase = ''
    # Set up HOME directory
    export HOME=$TMPDIR

    # Compile JavaScript to WASI p1 module using Javy
    javy build ${mainFile} -o ${name}_module.wasm

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