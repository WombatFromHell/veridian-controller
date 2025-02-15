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
        cargoToml = builtins.readFile ./Cargo.toml;
        versionMatch = builtins.match ''.*version[[:space:]]*=[[:space:]]*"([0-9]+\.[0-9]+\.[0-9]+)".*'' cargoToml;
      in {
        packages = {
          veridian-controller = naersk'.buildPackage {
            pname = "veridian-controller";
            version =
              if versionMatch != null
              then builtins.elemAt versionMatch 0
              else throw "Could not extract version from Cargo.toml. Make sure it contains a line like `version = \"0.1.0\"`.";
            src = ./.;
          };
          default = self.packages.${system}.veridian-controller;
        };

        apps.default = flake-utils.lib.mkApp {
          drv = self.packages.${system}.default;
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
