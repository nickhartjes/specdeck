{
  description = "SpecDeck: run a coding agent with your team watching along";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  # Builds the Rust dependencies in a derivation of their own, which CI caches (nix/package.nix).
  inputs.crane.url = "github:ipetkov/crane";

  # Linux only: macOS installs through Homebrew (design decision 13).
  outputs =
    {
      self,
      nixpkgs,
      crane,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forEachSystem = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forEachSystem (pkgs: rec {
        specdeck = pkgs.callPackage ./nix/package.nix {
          src = self;
          craneLib = crane.mkLib pkgs;
        };
        default = specdeck;
      });

      apps = forEachSystem (pkgs: {
        default = {
          type = "app";
          program = nixpkgs.lib.getExe self.packages.${pkgs.stdenv.hostPlatform.system}.specdeck;
        };
      });

      overlays.default = final: _prev: {
        specdeck = final.callPackage ./nix/package.nix {
          src = self;
          craneLib = crane.mkLib final;
        };
      };

      checks = forEachSystem (pkgs: {
        specdeck = self.packages.${pkgs.stdenv.hostPlatform.system}.specdeck;
      });
    };
}
