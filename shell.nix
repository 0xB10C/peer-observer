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

      # for code coverage:
      pkgs.cargo-tarpaulin

      # for integration tests
      pkgs.bitcoind
    ];

    shellHook = ''
      # during the integration tests, don't try to download a bitcoind binary
      # use the nix one instead
      export BITCOIND_SKIP_DOWNLOAD=1
      export BITCOIND_EXE=${pkgs.bitcoind}/bin/bitcoind
    '';

    # Use for running integration tests
    NATS_SERVER_BINARY = "${pkgs.nats-server}/bin/nats-server";
}
