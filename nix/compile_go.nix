# Compilation function for Go WASM programs using TinyGo
{ pkgs }:

{ name, src, mainFile ? "main.go" }:
pkgs.stdenv.mkDerivation {
  name = "${name}-wasm";
  inherit src;

  nativeBuildInputs = with pkgs; [
    tinygo
    wasm-tools
  ];

  buildPhase = ''
    # Set up HOME directory for TinyGo
    export HOME=$TMPDIR

    # Compile Go to WASIp2 component using TinyGo
    tinygo build -target=wasip2 -o ${name}.wasm ${mainFile}
  '';

  installPhase = ''
    mkdir -p $out
    cp ${name}.wasm $out/
  '';
}