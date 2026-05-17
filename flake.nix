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

        fenixPkgs = fenix.packages.${system};

        # Git SHA threaded into both the web bundle (`__WEB_GIT_SHA__`)
        # and the shell binary (`option_env!("VICTRON_CONTROLLER_GIT_SHA")`).
        # Without this, Nix builds drop to the build.rs/build-web.sh
        # `git rev-parse` fallback, which fails in the Nix sandbox
        # (no `.git/`, no git binary in PATH) — and `/api/version`
        # reports `git_sha: null`, breaking the dashboard's
        # version-reload feature.
        # `self.rev` is set on clean trees; `self.dirtyRev` on dirty
        # trees (Nix ≥ 2.16). Match build.rs's `--short=12` length so
        # client and server SHAs compare equal in the dashboard's
        # version-mismatch reload check.
        gitRev = self.rev or self.dirtyRev or null;
        gitSha = if gitRev != null then builtins.substring 0 12 gitRev else "";

        # Rust toolchain with both host and ARMv7-Venus cross targets.
        # Fenix provides per-target std, which stock nixpkgs rustc doesn't.
        # Single source of truth — used by the dev shell AND the package
        # derivations so shell builds and sandbox builds use the same
        # compiler bits.
        rustToolchain = fenixPkgs.combine [
          fenixPkgs.stable.cargo
          fenixPkgs.stable.rustc
          fenixPkgs.stable.clippy
          fenixPkgs.stable.rustfmt
          fenixPkgs.stable.rust-analyzer
          fenixPkgs.targets.armv7-unknown-linux-gnueabihf.stable.rust-std
        ];

        # Cross linker for armv7 — pulled from nixpkgs's cross toolchain.
        crossCC = pkgs.pkgsCross.armv7l-hf-multiplatform.stdenv.cc;

        # Cargo.lock has no `git+` sources (verified) → no outputHashes.
        cargoLock = { lockFile = ./Cargo.lock; };

        # Native rustPlatform keyed on the fenix toolchain — same compiler
        # in `nix develop` and `nix build`.
        rustPlatformNative = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        # Cross rustPlatform: keys cargoBuildHook etc. on the armv7 cross
        # stdenv (so `--target armv7-unknown-linux-gnueabihf` is passed
        # automatically and the install hook reads from the right
        # `target/<triple>/release/` subdir), while the compiler bits
        # come from the same fenix toolchain used in the dev shell.
        rustPlatformArmv7 = pkgs.pkgsCross.armv7l-hf-multiplatform.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        # Web bundle: tsc typecheck + esbuild minified bundle. Source is
        # `./web` only — no npm involved. The two devDependencies
        # (`esbuild`, `typescript`) are pulled directly from nixpkgs;
        # there is no `node_modules`, no `package-lock.json`. The
        # output `bundle.js` is consumed by the Rust derivation via
        # `postPatch` since the shell crate `include_str!`s it.
        web-bundle = pkgs.stdenv.mkDerivation {
          pname = "victron-controller-web-bundle";
          version = "0.1.0";
          src = ./web;
          nativeBuildInputs = [ pkgs.esbuild pkgs.typescript ];
          VICTRON_CONTROLLER_GIT_SHA = gitSha;
          buildPhase = ''
            runHook preBuild
            tsc --noEmit
            esbuild src/index.ts --bundle --minify --outfile=bundle.js \
              "--define:__WEB_GIT_SHA__=\"$VICTRON_CONTROLLER_GIT_SHA\""
            runHook postBuild
          '';
          installPhase = ''
            runHook preInstall
            mkdir -p $out
            cp bundle.js $out/bundle.js
            runHook postInstall
          '';
        };

        # Shared cargo invocation: build only the shell crate's binary.
        # `bundle.js` is gitignored — `postPatch` copies it from the
        # web-bundle derivation before cargo runs. baboon-generated
        # sources stay committed (no IFD via the baboon flake input).
        commonAttrs = {
          pname = "victron-controller";
          version = "0.1.0";
          src = ./.;
          inherit cargoLock;
          cargoBuildFlags = [ "-p" "victron-controller-shell" ];
          # Bake the SHA into the shell binary so `/api/version` and the
          # WebSocket `Hello` carry a non-null value (see `gitSha`
          # comment above). build.rs treats "" as absent.
          VICTRON_CONTROLLER_GIT_SHA = gitSha;
          postPatch = ''
            cp ${web-bundle}/bundle.js crates/shell/static/bundle.js
          '';
        };

        victron-controller = rustPlatformNative.buildRustPackage (commonAttrs // {
          # Native build runs the workspace tests.
          doCheck = true;
        });

        victron-controller-armv7 = rustPlatformArmv7.buildRustPackage (commonAttrs // {
          pname = "victron-controller-armv7";

          # cc-rs / ring native-build-scripts env matrix — verbatim from
          # the dev shell. The cross stdenv would set CC_… on its own,
          # but we pin the exact gcc binary fenix is paired with so the
          # `+neon`/`+vfp3` flags below match what the cc-rs invocations
          # produce.
          CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER =
            "${crossCC}/bin/${crossCC.targetPrefix}gcc";
          CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_RUSTFLAGS =
            "-C target-feature=+v7,+vfp3,+neon";
          CC_armv7_unknown_linux_gnueabihf = "${crossCC}/bin/${crossCC.targetPrefix}gcc";
          AR_armv7_unknown_linux_gnueabihf = "${crossCC}/bin/${crossCC.targetPrefix}ar";
          CXX_armv7_unknown_linux_gnueabihf = "${crossCC}/bin/${crossCC.targetPrefix}g++";

          # Host C compiler so build-script `cc-rs` invocations targeting
          # the build platform still find a `$CC`.
          depsBuildBuild = [ pkgs.buildPackages.stdenv.cc ];

          nativeBuildInputs = [ pkgs.patchelf pkgs.upx ];

          # Host can't execute armv7 binaries — skip cross tests.
          doCheck = false;

          # postFixup:
          #   1. Rewrite the ELF interpreter to the path Venus provides.
          #      Without this the binary points at a /nix/store path that
          #      doesn't exist on the target.
          #   2. UPX-compress the binary in-place. `--lzma -9` is
          #      deterministic for a given (input, UPX version) pair, so
          #      pinning UPX via nixpkgs gives reproducible output. Cuts
          #      the ARMv7 ELF from ~8 MiB to ~2 MiB — directly halves
          #      the bytes pushed on every deploy and the eMMC write
          #      pressure on the Venus.
          # Note: empty rpath is written as `""` because Nix indented
          # strings treat `''` as an escape sequence for `''`.
          # Note: chmod u+w before UPX because /nix/store binaries are
          # 0555 and UPX needs to rewrite the file in place.
          postFixup = ''
            patchelf \
              --set-interpreter /lib/ld-linux-armhf.so.3 \
              --set-rpath "" \
              $out/bin/victron-controller
            chmod u+w $out/bin/victron-controller
            upx --lzma -q -9 $out/bin/victron-controller
            chmod a-w $out/bin/victron-controller
          '';
        });
      in {
        packages = {
          inherit victron-controller victron-controller-armv7 web-bundle;
          default = victron-controller;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            cargo-nextest
            shellcheck
            crossCC
            upx        # optional binary compression — shrinks the 4-5 MB
                       # release ELF to ~1.5 MB, halving the bytes
                       # pushed on every dev-cycle scp and eMMC write.
            baboon.packages.${system}.baboon  # data model compiler for
                                              # the dashboard wire format
            # Frontend build toolchain — invoked directly by
            # scripts/build-web.sh (no npm). The `web-bundle` Nix
            # derivation uses the same binaries.
            esbuild
            typescript
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

          # PR-DIAG-1: tikv-jemalloc-sys runs an autotools `configure`
          # that compiles probes with `-O0 -Werror`. Nix's default cc
          # wrapper injects `-D_FORTIFY_SOURCE=2`, and glibc emits a
          # warning ("FORTIFY_SOURCE requires compiling with optimization")
          # when `-O0` is set, which `-Werror` upgrades to a build
          # failure. Dropping the fortify hardening flags from the
          # wrapper unbreaks debug builds; release builds compile with
          # `-O*` and would not have triggered the warning anyway.
          hardeningDisable = [ "fortify" "fortify3" ];

          shellHook = ''
            echo "victron-controller dev shell"
            echo "  build (host):   nix build .#victron-controller"
            echo "  build (armv7):  nix build .#victron-controller-armv7"
            echo "  web bundle:     nix build .#web-bundle    (or ./scripts/build-web.sh for --watch)"
            echo "  deploy:         ./scripts/install-victron.sh user@venus-host"
            echo "  cargo test:     run ./scripts/build-web.sh once if crates/shell/static/bundle.js is missing"
          '';
        };
      });
}
