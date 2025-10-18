# Compilation function for Rust WASM examples
{ craneLib }:

{ name, src }:
craneLib.buildPackage {
  name = "${name}-wasm";
  inherit src;
  inheritToolchain = false;

  CARGO_BUILD_TARGET = "wasm32-wasip2";
  doCheck = false;

  installPhase = ''
    mkdir -p $out
    cp -v target/wasm32-wasip2/release/${name}.wasm $out/
  '';
}