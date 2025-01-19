{ pkgs ? import <nixpkgs> {}}:
#let
#  unstable = import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/nixos-unstable.tar.gz") {};
#in
pkgs.mkShell {

    hardeningDisable = [ "stackprotector" "fortify" ];

    buildInputs = [
      pkgs.rustc
      pkgs.cargo
      pkgs.cmake
      pkgs.protobuf

      pkgs.rustfmt

      pkgs.bpftools

      # libbpf CO-RE pkgs
      pkgs.clang_14
      pkgs.llvm
      pkgs.elfutils
      pkgs.zlib
      pkgs.pkg-config
      pkgs.which
    ];
}
