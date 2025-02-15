{pkgs ? import <nixpkgs> {}}: let
  # Import the necessary dependencies and variables from the original flake.nix
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
in
  pkgs.mkShell {
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
  }
