# Build counts tool from GitHub release
{ pkgs, craneLib }:

let
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
}