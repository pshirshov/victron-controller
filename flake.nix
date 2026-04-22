{
  description = "victron-controller — on-device Rust service replacing Node-RED flows";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    baboon.url = "github:7mind/baboon";
  };

  outputs = { self, nixpkgs, flake-utils, fenix, baboon }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        # Rust toolchain with both host and ARMv7-Venus cross targets.
        # Fenix provides per-target std, which stock nixpkgs rustc doesn't.
        rustToolchain = fenix.packages.${system}.combine [
          fenix.packages.${system}.stable.cargo
          fenix.packages.${system}.stable.rustc
          fenix.packages.${system}.stable.clippy
          fenix.packages.${system}.stable.rustfmt
          fenix.packages.${system}.stable.rust-analyzer
          fenix.packages.${system}.targets.armv7-unknown-linux-gnueabihf.stable.rust-std
        ];

        # Cross linker for armv7 — pulled from nixpkgs's cross toolchain.
        crossCC = pkgs.pkgsCross.armv7l-hf-multiplatform.stdenv.cc;
      in {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            cargo-nextest
            shellcheck
            crossCC
            patchelf   # used by install-victron.sh to rewrite the ELF
                       # interpreter path from the nix-store default
                       # to /lib/ld-linux-armhf.so.3 for Venus.
            baboon.packages.${system}.baboon  # data model compiler for
                                              # the dashboard wire format
          ];

          # Tell cargo which linker to use when targeting armv7.
          # `cargo build --target armv7-unknown-linux-gnueabihf` now works
          # inside `nix develop`.
          CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER =
            "${crossCC}/bin/${crossCC.targetPrefix}gcc";
          CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_RUSTFLAGS =
            "-C target-feature=+v7,+vfp3,+neon";
          # cc-rs (ring/native build-scripts) needs to know the cross CC
          # explicitly — host gcc doesn't recognise -mfpu=vfpv3-d16.
          CC_armv7_unknown_linux_gnueabihf = "${crossCC}/bin/${crossCC.targetPrefix}gcc";
          AR_armv7_unknown_linux_gnueabihf = "${crossCC}/bin/${crossCC.targetPrefix}ar";
          CXX_armv7_unknown_linux_gnueabihf = "${crossCC}/bin/${crossCC.targetPrefix}g++";

          RUST_BACKTRACE = "1";
          RUST_LOG = "debug";

          shellHook = ''
            echo "victron-controller dev shell"
            echo "  native build:  cargo build"
            echo "  armv7 build:   cargo build --target armv7-unknown-linux-gnueabihf --release"
            echo "  deploy:        ./scripts/install-victron.sh user@venus-host"
          '';
        };
      });
}
