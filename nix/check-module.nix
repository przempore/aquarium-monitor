{ nixpkgs, pkgs, module }:

let
  evaluated = nixpkgs.lib.nixosSystem {
    inherit (pkgs) system;
    modules = [
      module
      {
        services.aquarium-monitor.enable = true;
        services.aquarium-monitor.device = "/dev/ttyUSB0";
         services.aquarium-monitor.temperaturePath = "/sys/bus/w1/devices/28-test/w1_slave";
         services.aquarium-monitor.intervalSeconds = 2;
         services.aquarium-monitor.influxdb.enable = true;
         services.aquarium-monitor.influxdb.environmentFile = "/run/keys/influxdb-environment";
         services.aquarium-monitor.influxdb.tokenFile = "/run/keys/influxdb-token";
         services.aquarium-monitor.grafana.enable = true;
      }
    ];
  };
  service = evaluated.config.systemd.services.aquarium-monitor;
in
assert service.serviceConfig.StandardInput == "null";
assert builtins.match ".*--device.*" service.serviceConfig.ExecStart != null;
assert builtins.match ".*--temperature-path.*" service.serviceConfig.ExecStart != null;
assert builtins.match ".*--interval-seconds 2.*" service.serviceConfig.ExecStart != null;
assert builtins.match ".*--influx-url http://127.0.0.1:8086.*" service.serviceConfig.ExecStart != null;
assert builtins.match ".*--influx-token-file /run/credentials/aquarium-monitor.service/influxdb-token.*" service.serviceConfig.ExecStart != null;
assert builtins.match ".*secret.*" service.serviceConfig.ExecStart == null;
assert builtins.match ".*stty.*9600.*" service.serviceConfig.ExecStartPre != null;
assert service.serviceConfig.Restart == "on-failure";
assert evaluated.config.virtualisation.oci-containers.containers.influxdb.ports == [ "127.0.0.1:8086:8086" ];
assert evaluated.config.virtualisation.oci-containers.containers.grafana.ports == [ "127.0.0.1:3000:3000" ];
assert evaluated.config.virtualisation.oci-containers.containers.influxdb.environmentFiles == [ "/run/keys/influxdb-environment" ];
pkgs.runCommand "aquarium-monitor-module-evaluation" { } "touch $out"
