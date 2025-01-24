{ pkgs ? import <nixpkgs> {} }:
  pkgs.mkShell {
    nativeBuildInputs = [
      pkgs.python39Packages.nats-py
      pkgs.python39Packages.protobuf
    ];
}
