{ nixpkgs, pkgs, module }:

let
  evaluatedDisabled = nixpkgs.lib.nixosSystem {
    inherit (pkgs) system;
    modules = [ module ];
  };
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
         services.aquarium-monitor.influxdb.retention = "30d";
         services.aquarium-monitor.influxdb.healthCheck.enable = true;
          services.aquarium-monitor.grafana.enable = true;
         services.aquarium-monitor.grafana.provisioning = {
            enable = true;
            tokenEnvironmentFile = "/run/keys/grafana-influxdb-token-environment";
          };
           services.aquarium-monitor.grafana.dashboard.enable = true;
           services.aquarium-monitor.grafana.healthCheck.enable = true;
        }
       ];
    };
   evaluatedSimulator = nixpkgs.lib.nixosSystem {
     inherit (pkgs) system;
     modules = [
       module
       {
         services.aquarium-monitor.simulator = {
           enable = true;
           tankId = "tank-sim";
           temperatureC = 26.5;
           intervalSeconds = 3;
           rawLogPath = "/var/lib/aquarium-monitor/simulator.ndjson";
         };
         services.aquarium-monitor.influxdb = {
           enable = true;
           environmentFile = "/run/keys/influxdb-environment";
           tokenFile = "/run/keys/influxdb-token";
         };
         services.aquarium-monitor.grafana.listenAddress = "100.64.0.10";
         services.aquarium-monitor.grafana.enable = true;
       }
     ];
   };
   service = evaluated.config.systemd.services.aquarium-monitor;
   simulatorService = evaluatedSimulator.config.systemd.services.aquarium-monitor-simulator;
  grafanaVolumes = evaluated.config.virtualisation.oci-containers.containers.grafana.volumes;
  datasourceVolume = builtins.head (builtins.filter (volume: builtins.match ".*aquarium-monitor-grafana-datasource.yml.*" volume != null) grafanaVolumes);
  dashboardProviderVolume = builtins.head (builtins.filter (volume: builtins.match ".*dashboard-provider.yml.*" volume != null) grafanaVolumes);
  dashboardVolume = builtins.head (builtins.filter (volume: builtins.match ".*aquarium-monitor-grafana-dashboard.json.*" volume != null) grafanaVolumes);
  dashboard = builtins.fromJSON (builtins.readFile (builtins.head (pkgs.lib.splitString ":" dashboardVolume)));
in
assert !evaluatedDisabled.config.services.aquarium-monitor.grafana.dashboard.enable;
assert !(builtins.hasAttr "grafana" evaluatedDisabled.config.virtualisation.oci-containers.containers);
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
   assert evaluated.config.virtualisation.oci-containers.containers.influxdb.environment.DOCKER_INFLUXDB_INIT_RETENTION == "30d";
   assert builtins.elem "--health-cmd=influx ping --host http://127.0.0.1:8086" evaluated.config.virtualisation.oci-containers.containers.influxdb.extraOptions;
   assert builtins.elem "--health-cmd=wget --spider --quiet http://127.0.0.1:3000/api/health" evaluated.config.virtualisation.oci-containers.containers.grafana.extraOptions;
   assert builtins.elem "d '/var/lib/aquarium-monitor/influxdb' 0750 1000 1000 -" evaluated.config.systemd.tmpfiles.rules;
   assert builtins.elem "d '/var/lib/aquarium-monitor/grafana' 0750 472 472 -" evaluated.config.systemd.tmpfiles.rules;
 assert evaluated.config.virtualisation.oci-containers.containers.grafana.ports == [ "127.0.0.1:3000:3000" ];
 assert evaluatedSimulator.config.virtualisation.oci-containers.containers.grafana.ports == [ "100.64.0.10:3000:3000" ];
assert builtins.match ".*:/etc/grafana/provisioning/datasources/aquarium-monitor.yml:ro" datasourceVolume != null;
assert builtins.match ".*:/etc/grafana/provisioning/dashboards/aquarium-monitor.yml:ro" dashboardProviderVolume != null;
assert builtins.match ".*:/etc/grafana/provisioning/dashboards/aquarium-monitor.json:ro" dashboardVolume != null;
assert builtins.substring 0 11 (builtins.head (builtins.filter (volume: builtins.match ".*:ro" volume != null) grafanaVolumes)) == "/nix/store/";
assert evaluated.config.virtualisation.oci-containers.containers.grafana.environmentFiles == [ "/run/keys/grafana-influxdb-token-environment" ];
assert builtins.match ".*INFLUXDB_TOKEN.*" (builtins.readFile (builtins.head (pkgs.lib.splitString ":" datasourceVolume))) != null;
assert builtins.match ".*test-token.*" (builtins.readFile (builtins.head (pkgs.lib.splitString ":" datasourceVolume))) == null;
assert builtins.match ".*uid: aquarium-influxdb.*" (builtins.readFile (builtins.head (pkgs.lib.splitString ":" datasourceVolume))) != null;
assert dashboard.uid == "aquarium-monitor";
 assert builtins.length dashboard.panels == 7;
assert builtins.match ".*tank_id.*" (builtins.readFile (builtins.head (pkgs.lib.splitString ":" dashboardVolume))) != null;
assert builtins.match ".*ec_us_cm.*" (builtins.readFile (builtins.head (pkgs.lib.splitString ":" dashboardVolume))) != null;
 assert builtins.match ".*temp_c.*" (builtins.readFile (builtins.head (pkgs.lib.splitString ":" dashboardVolume))) != null;
 assert builtins.match ".*Telemetry freshness.*" (builtins.readFile (builtins.head (pkgs.lib.splitString ":" dashboardVolume))) != null;
 assert builtins.match ".*source.*quality.*" (builtins.readFile (builtins.head (pkgs.lib.splitString ":" dashboardVolume))) != null;
 assert builtins.match ".*aquarium_alarm.*rule_id.*observed_value.*" (builtins.readFile (builtins.head (pkgs.lib.splitString ":" dashboardVolume))) != null;
 assert builtins.match ".*--influx-alarm-output.*" (builtins.readFile (builtins.head (pkgs.lib.splitString ":" dashboardVolume))) != null;
 assert builtins.all (uid: uid == "aquarium-influxdb") (builtins.concatLists (map (panel: map (target: target.datasource.uid) panel.targets) dashboard.panels));
   assert evaluated.config.virtualisation.oci-containers.containers.influxdb.environmentFiles == [ "/run/keys/influxdb-environment" ];
   assert service.serviceConfig.DynamicUser;
   assert service.serviceConfig.StateDirectory == "aquarium-monitor";
 assert service.serviceConfig.StateDirectoryMode == "0750";
 assert builtins.match ".*simulator-ezo-ec.*--continuous.*--interval-seconds 3.*" (builtins.readFile simulatorService.serviceConfig.ExecStart) != null;
 assert builtins.match ".*collector.*--raw-log /var/lib/aquarium-monitor/simulator.ndjson.*--tank-id tank-sim.*--sim-temperature-c 26.5.*" (builtins.readFile simulatorService.serviceConfig.ExecStart) != null;
 assert builtins.match ".*--influx-url http://127.0.0.1:8086.*" (builtins.readFile simulatorService.serviceConfig.ExecStart) != null;
 assert builtins.match ".*--influx-batch-size 1.*" (builtins.readFile simulatorService.serviceConfig.ExecStart) != null;
 assert simulatorService.serviceConfig.LoadCredential == [ "influxdb-token:/run/keys/influxdb-token" ];
 assert builtins.elem "podman-influxdb.service" simulatorService.requires;
 assert builtins.elem "podman-influxdb.service" simulatorService.after;
 assert evaluatedSimulator.config.virtualisation.oci-containers.containers.influxdb.ports == [ "127.0.0.1:8086:8086" ];
 assert builtins.any (assertion: assertion.message == "services.aquarium-monitor.enable and services.aquarium-monitor.simulator.enable cannot both be enabled") evaluated.config.assertions;
 pkgs.runCommand "aquarium-monitor-module-evaluation" { } "touch $out"
