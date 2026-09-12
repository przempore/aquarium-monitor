{ nixpkgs, pkgs, module }:

let
  evaluated = nixpkgs.lib.nixosSystem {
    inherit (pkgs) system;
    modules = [
      module
      {
        services.aquarium-monitor.enable = true;
      }
    ];
  };
  service = evaluated.config.systemd.services.aquarium-monitor;
in
assert service.serviceConfig.StandardInput == "null";
assert service.serviceConfig.Restart == "on-failure";
pkgs.runCommand "aquarium-monitor-module-evaluation" { } "touch $out"
