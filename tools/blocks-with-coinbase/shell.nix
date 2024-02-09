{ pkgs ? import <nixpkgs> {} }:
  pkgs.mkShell {
    nativeBuildInputs = [
      pkgs.python3Packages.nanomsg-python
      pkgs.python3Packages.protobuf
    ];
}
