{
  inputs = {
    nixpkgs.url = "github:cachix/devenv-nixpkgs/rolling";
    systems.url = "github:nix-systems/default";

    devenv.url = "github:cachix/devenv";
    devenv.inputs.nixpkgs.follows = "nixpkgs";

    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs = { nixpkgs.follows = "nixpkgs"; };

    cargo2nix.url = "github:cargo2nix/cargo2nix";
    cargo2nix.inputs.nixpkgs.follows = "nixpkgs";
    cargo2nix.inputs.rust-overlay.follows = "rust-overlay";

    flake-utils.follows = "cargo2nix/flake-utils";
  };

  nixConfig = {
    extra-trusted-public-keys = "devenv.cachix.org-1:w1cLUi8dv3hnoSPGAuibQv+f9TZLr6cv/Hm9XgU50cw=";
    extra-substituters = "https://devenv.cachix.org";
  };

  outputs = inputs: with inputs;

    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ cargo2nix.overlays.default ];
        };
      in
      {
        formatter = pkgs.nixpkgs-fmt;
        packages =
          let
            rustPkgs = pkgs.rustBuilder.makePackageSet {
              rustChannel = "nightly";
              rustVersion = "2026-04-04";
              packageFun = import ./Cargo.nix;
            };
          in rec
          {
            simulator = rustPkgs.workspace.simulator-ezo-ec { };
            default = simulator;
          };
        devShells =
          {
            default = devenv.lib.mkShell {
              inherit inputs pkgs;
              modules = [
                {
                  # https://devenv.sh/reference/options/
                  packages = [
                    pkgs.nixd
                    inputs.cargo2nix.packages.${pkgs.system}.default
                  ];

                  languages.rust = {
                    enable = true;
                    channel = "nightly";
                  };

                  enterShell = ''
                  '';
                }
              ];
            };
          };
      });
}
