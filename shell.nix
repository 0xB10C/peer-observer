{ pkgs ? import <nixpkgs> {}}:
let
  unstable = import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/nixos-unstable.tar.gz") {};
in
pkgs.mkShell {

    hardeningDisable = [ "stackprotector" "fortify" ];

    buildInputs = [
      pkgs.linuxPackages.bcc
      pkgs.cargo
      pkgs.cmake
      pkgs.protobuf

      pkgs.rustfmt

      pkgs.bpftool

      # libbpf CO-RE pkgs
      unstable.clang_14
      pkgs.llvm
      pkgs.elfutils
      pkgs.zlib
      pkgs.pkg-config
      pkgs.which
    ];
}
