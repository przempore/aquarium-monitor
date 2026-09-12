{ nixpkgs, pkgs, module }:

let
  evaluated = nixpkgs.lib.nixosSystem {
    inherit (pkgs) system;
    modules = [
      module
      {
        services.aquarium-monitor.enable = true;
        services.aquarium-monitor.device = "/dev/ttyUSB0";
        services.aquarium-monitor.intervalSeconds = 2;
      }
    ];
  };
  service = evaluated.config.systemd.services.aquarium-monitor;
in
assert service.serviceConfig.StandardInput == "null";
assert builtins.match ".*--device.*" service.serviceConfig.ExecStart != null;
assert builtins.match ".*--interval-seconds 2.*" service.serviceConfig.ExecStart != null;
assert builtins.match ".*stty.*9600.*" service.serviceConfig.ExecStartPre != null;
assert service.serviceConfig.Restart == "on-failure";
pkgs.runCommand "aquarium-monitor-module-evaluation" { } "touch $out"
