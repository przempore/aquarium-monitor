{ defaultPackage ? (_: null), lib, pkgs, config, ... }:

let
  cfg = config.services.aquarium-monitor;
  collectorService = "aquarium-monitor.service";
in
{
  options.services.aquarium-monitor = {
    enable = lib.mkEnableOption "the Aquarium Monitor collector";

    package = lib.mkOption {
      type = lib.types.package;
      default =
        let package = defaultPackage pkgs.system;
        in if package == null then
          throw "services.aquarium-monitor.package must be set when importing the module directly"
        else package;
      description = "Collector package to run.";
    };

    rawLogPath = lib.mkOption {
      type = lib.types.path;
      default = "/var/lib/aquarium-monitor/raw-frames.ndjson";
      description = "Path to the append-only raw frame NDJSON log.";
    };

    device = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = "Linux serial device path for an EZO-EC probe.";
    };

    tankId = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Required stable tank identity for the hardware collector service.";
    };

    temperaturePath = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = "Linux w1_slave path for a DS18B20 temperature sensor.";
    };

    intervalSeconds = lib.mkOption {
      type = lib.types.ints.positive;
      default = 1;
      description = "Seconds between EZO-EC read requests.";
    };

    baudRate = lib.mkOption {
      type = lib.types.ints.positive;
      default = 9600;
      description = "Serial baud rate. Only 9600 is currently supported.";
    };

    influxdb = {
      enable = lib.mkEnableOption "a local InfluxDB 2.x OCI container";
      image = lib.mkOption { type = lib.types.str; default = "influxdb:2.7"; description = "InfluxDB image and tag."; };
      dataDir = lib.mkOption { type = lib.types.str; default = "/var/lib/aquarium-monitor/influxdb"; description = "Persistent InfluxDB data directory."; };
      port = lib.mkOption { type = lib.types.port; default = 8086; description = "Local InfluxDB host port."; };
      organization = lib.mkOption { type = lib.types.str; default = "aquarium"; description = "InfluxDB organization."; };
      bucket = lib.mkOption { type = lib.types.str; default = "telemetry"; description = "InfluxDB bucket."; };
      environmentFile = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        description = "Uncommitted EnvironmentFile for InfluxDB initialization.";
      };
      tokenFile = lib.mkOption {
        type = lib.types.path;
        default = "/run/keys/aquarium-monitor-influxdb-token";
        description = "Uncommitted file containing the collector token.";
      };
    };

    grafana = {
      enable = lib.mkEnableOption "a local Grafana OCI container";
      image = lib.mkOption { type = lib.types.str; default = "grafana/grafana:11.5.2"; description = "Grafana image and tag."; };
      dataDir = lib.mkOption { type = lib.types.str; default = "/var/lib/aquarium-monitor/grafana"; description = "Persistent Grafana data directory."; };
      port = lib.mkOption { type = lib.types.port; default = 3000; description = "Local Grafana host port."; };
    };
  };

  config = lib.mkMerge [
    (lib.mkIf (cfg.influxdb.enable || cfg.grafana.enable) {
      virtualisation.oci-containers.backend = "podman";
    })
    (lib.mkIf cfg.influxdb.enable {
      assertions = [
        {
          assertion = cfg.influxdb.environmentFile != null;
          message = "services.aquarium-monitor.influxdb.environmentFile must be set when InfluxDB is enabled";
        }
        {
          assertion = cfg.influxdb.organization != "" && cfg.influxdb.bucket != "";
          message = "InfluxDB organization and bucket must not be empty";
        }
      ];
      virtualisation.oci-containers.containers.influxdb = {
        image = cfg.influxdb.image;
        ports = [ "127.0.0.1:${toString cfg.influxdb.port}:8086" ];
        volumes = [ "${cfg.influxdb.dataDir}:/var/lib/influxdb2" ];
        environmentFiles = lib.optional (cfg.influxdb.environmentFile != null) cfg.influxdb.environmentFile;
        extraOptions = [ "--pull=missing" "--restart=on-failure" ];
      };
    })
    (lib.mkIf cfg.grafana.enable {
      virtualisation.oci-containers.containers.grafana = {
        image = cfg.grafana.image;
        ports = [ "127.0.0.1:${toString cfg.grafana.port}:3000" ];
        volumes = [ "${cfg.grafana.dataDir}:/var/lib/grafana" ];
        dependsOn = lib.optional cfg.influxdb.enable "influxdb";
        extraOptions = [ "--pull=missing" "--restart=on-failure" ];
      };
    })
    (lib.mkIf cfg.enable {
      assertions = [
        { assertion = cfg.device != null; message = "services.aquarium-monitor.device must be set for the hardware collector service"; }
        { assertion = cfg.tankId != null && cfg.tankId != ""; message = "services.aquarium-monitor.tankId must be set for the hardware collector service"; }
        { assertion = cfg.temperaturePath != null; message = "services.aquarium-monitor.temperaturePath must be set for the hardware collector service"; }
        { assertion = cfg.baudRate == 9600; message = "services.aquarium-monitor.baudRate must be 9600; arbitrary baud rates are not supported yet"; }
      ] ++ lib.optional cfg.influxdb.enable {
        assertion = cfg.influxdb.tokenFile != null;
        message = "services.aquarium-monitor.influxdb.tokenFile must be set for collector InfluxDB output";
      };

      systemd.services.aquarium-monitor = {
        description = "Aquarium Monitor collector";
        wantedBy = [ "multi-user.target" ];
        wants = lib.optional cfg.influxdb.enable "podman-influxdb.service";
        requires = lib.optional cfg.influxdb.enable "podman-influxdb.service";
        after = [ "local-fs.target" ] ++ lib.optional cfg.influxdb.enable "podman-influxdb.service";
        serviceConfig = {
          ExecStart = lib.escapeShellArgs ([
            "${cfg.package}/bin/collector" "--raw-log" cfg.rawLogPath
            "--device" cfg.device "--temperature-path" cfg.temperaturePath
            "--tank-id" cfg.tankId
            "--interval-seconds" (toString cfg.intervalSeconds)
          ] ++ lib.optionals cfg.influxdb.enable [
            "--influx-url" "http://127.0.0.1:${toString cfg.influxdb.port}"
            "--influx-organization" cfg.influxdb.organization
            "--influx-bucket" cfg.influxdb.bucket
            "--influx-token-file" "/run/credentials/${collectorService}/influxdb-token"
          ]);
          ExecStartPre = lib.escapeShellArgs [
            "${pkgs.coreutils}/bin/stty" "-F" cfg.device "9600" "raw" "-echo"
            "-ixon" "-ixoff" "-crtscts" "min" "1" "time" "10"
          ];
          LoadCredential = lib.optional cfg.influxdb.enable "influxdb-token:${cfg.influxdb.tokenFile}";
          StandardInput = "null";
          Restart = "on-failure";
          RestartSec = 5;
          StateDirectory = "aquarium-monitor";
          DynamicUser = true;
          NoNewPrivileges = true;
          PrivateTmp = true;
          ProtectHome = true;
          ProtectSystem = "strict";
        };
      };
    })
  ];
}
