{
  description = "Veridian Controller";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
  };

  nixConfig = {
    extra-substituters = [
      "https://wombatfromhell.cachix.org/"
    ];
    extra-trusted-public-keys = [
      "wombatfromhell.cachix.org-1:pyIVJJkoLxkjH/MKK1ylrrdJKPpm+aXLeD2zAqVk9lA="
    ];
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    naersk,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {inherit system;};
        naersk' = pkgs.callPackage naersk {};
        overrides = builtins.fromTOML (builtins.readFile ./rust-toolchain.toml);
        rustcVersion = overrides.toolchain.channel;
        llvmPackagesLatest = pkgs.llvmPackages_latest;
        libPath = with pkgs;
          pkgs.lib.makeLibraryPath [
            # load external libraries that you need in your rust project here
          ];
        bindgenExtraClangArgs =
          # Includes normal include path
          (builtins.map (a: "-I${a}/include") [
            # add dev libraries here (e.g. pkgs.libvmi.dev)
            pkgs.glibc.dev
          ])
          # Includes with special directory paths
          ++ [
            "-I${llvmPackagesLatest.libclang.lib}/lib/clang/${llvmPackagesLatest.libclang.version}/include"
          ];
      in {
        packages = {
          veridian-controller = naersk'.buildPackage {
            pname = "veridian-controller";
            version = "0.2.7";
            src = ./.;
          };
          default = self.packages.${system}.veridian-controller;
        };

        apps.default = flake-utils.lib.mkApp {
          drv = self.packages.${system}.default;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            clang
            llvmPackagesLatest.bintools
            rustup
          ];
          RUSTC_VERSION = rustcVersion;
          LIBCLANG_PATH = pkgs.lib.makeLibraryPath [llvmPackagesLatest.libclang.lib];
          shellHook = ''
            export PATH=$PATH:$CARGO_HOME/bin
            export PATH=$PATH:$RUSTUP_HOME/toolchains/$RUSTC_VERSION-x86_64-unknown-linux-gnu/bin/
          '';
          RUSTFLAGS = builtins.map (a: "-L ${a}/lib") [
            # add libraries here (e.g. pkgs.libvmi)
          ];
          LD_LIBRARY_PATH = libPath;
          BINDGEN_EXTRA_CLANG_ARGS = bindgenExtraClangArgs;
        };
      }
    )
    // rec {
      overlays.default = final: prev: {
        inherit (self.packages.${prev.system}) veridian-controller;
      };

      nixosModules.default = {
        config,
        lib,
        pkgs,
        ...
      }: {
        config = {
          nixpkgs.overlays = [overlays.default];
        };
      };
    };
}
