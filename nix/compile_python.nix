# Configuration for Python WASM programs using pre-built runtime
{ pkgs, wasi-adapter }:

let
  # Fetch pre-built Python WASM runtime
  python-wasm-module = pkgs.fetchurl {
    url = "https://github.com/vmware-labs/webassembly-language-runtimes/releases/download/python%2F3.12.0%2B20231211-040d5a6/python-3.12.0.wasm";
    sha256 = "sha256-5dxaOYsHtU6o/bUDv2j7WD1TPxDsP5MJY+ArlQX3p2M=";
  };
in

{ name, src, scriptFile ? "app.py" }:
pkgs.stdenv.mkDerivation {
  name = "${name}-wasm";
  inherit src;

  nativeBuildInputs = with pkgs; [
    wasm-tools
  ];

  buildPhase = ''
    # The python-3.12.0.wasm is a WASI p1 module, convert to p2 component
    wasm-tools component new ${python-wasm-module} \
      --adapt wasi_snapshot_preview1=${wasi-adapter} \
      -o ${name}.wasm
  '';

  installPhase = ''
    mkdir -p $out
    cp ${name}.wasm $out/
    # Also copy the Python script for reference
    cp ${scriptFile} $out/
  '';
}