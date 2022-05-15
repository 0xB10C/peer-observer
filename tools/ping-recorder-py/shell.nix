{ pkgs ? import <nixpkgs> {} }:
  pkgs.mkShell {
    nativeBuildInputs = [
      pkgs.python39Packages.nanomsg-python
      pkgs.python39Packages.protobuf
    ];
}
