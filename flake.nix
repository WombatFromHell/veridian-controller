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
        version = pkgs.lib.strings.removeSuffix "\n" (builtins.readFile (pkgs.runCommand "get-version" {
            nativeBuildInputs = [pkgs.remarshal pkgs.jq];
          } ''
            toml2json ${./Cargo.toml} | jq -r '.package.version' > $out
          ''));
      in {
        packages = {
          veridian-controller = naersk'.buildPackage {
            pname = "veridian-controller";
            inherit version;
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
