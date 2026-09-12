{ nixpkgs, pkgs, module }:

let
  evaluated = nixpkgs.lib.nixosSystem {
    inherit (pkgs) system;
    modules = [
      module
      {
        services.aquarium-monitor.enable = true;
        services.aquarium-monitor.device = "/dev/ttyUSB0";
        services.aquarium-monitor.tankId = "tank-1";
         services.aquarium-monitor.temperaturePath = "/sys/bus/w1/devices/28-test/w1_slave";
         services.aquarium-monitor.intervalSeconds = 2;
         services.aquarium-monitor.influxdb.enable = true;
         services.aquarium-monitor.influxdb.environmentFile = "/run/keys/influxdb-environment";
          services.aquarium-monitor.influxdb.tokenFile = "/run/keys/influxdb-token";
          services.aquarium-monitor.grafana.enable = true;
          services.aquarium-monitor.grafana.provisioning = {
            enable = true;
            tokenEnvironmentFile = "/run/keys/grafana-influxdb-token-environment";
          };
       }
      ];
  };
  service = evaluated.config.systemd.services.aquarium-monitor;
  grafanaVolumes = evaluated.config.virtualisation.oci-containers.containers.grafana.volumes;
  datasourceVolume = builtins.head (builtins.filter (volume: builtins.match ".*aquarium-monitor-grafana-datasource.yml.*" volume != null) grafanaVolumes);
in
assert service.serviceConfig.StandardInput == "null";
assert builtins.match ".*--device.*" service.serviceConfig.ExecStart != null;
assert builtins.match ".*--tank-id tank-1.*" service.serviceConfig.ExecStart != null;
assert builtins.match ".*--temperature-path.*" service.serviceConfig.ExecStart != null;
assert builtins.match ".*--interval-seconds 2.*" service.serviceConfig.ExecStart != null;
assert builtins.match ".*--influx-url http://127.0.0.1:8086.*" service.serviceConfig.ExecStart != null;
assert builtins.match ".*--influx-token-file /run/credentials/aquarium-monitor.service/influxdb-token.*" service.serviceConfig.ExecStart != null;
assert builtins.match ".*secret.*" service.serviceConfig.ExecStart == null;
assert builtins.match ".*stty.*9600.*" service.serviceConfig.ExecStartPre != null;
assert service.serviceConfig.Restart == "on-failure";
assert evaluated.config.virtualisation.oci-containers.containers.influxdb.ports == [ "127.0.0.1:8086:8086" ];
  assert evaluated.config.virtualisation.oci-containers.containers.grafana.ports == [ "127.0.0.1:3000:3000" ];
  assert builtins.substring 0 11 (builtins.head (builtins.filter (volume: builtins.match ".*:ro" volume != null) grafanaVolumes)) == "/nix/store/";
  assert builtins.match ".*:/etc/grafana/provisioning/datasources/aquarium-monitor.yml:ro" datasourceVolume != null;
  assert evaluated.config.virtualisation.oci-containers.containers.grafana.environmentFiles == [ "/run/keys/grafana-influxdb-token-environment" ];
  assert builtins.match ".*INFLUXDB_TOKEN.*" (builtins.readFile (builtins.head (pkgs.lib.splitString ":" datasourceVolume))) != null;
  assert builtins.match ".*test-token.*" (builtins.readFile (builtins.head (pkgs.lib.splitString ":" datasourceVolume))) == null;
  assert evaluated.config.virtualisation.oci-containers.containers.influxdb.environmentFiles == [ "/run/keys/influxdb-environment" ];
pkgs.runCommand "aquarium-monitor-module-evaluation" { } "touch $out"
