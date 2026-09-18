{ defaultPackage ? (_: null), simulatorPackage ? (_: null), lib, pkgs, config, ... }:

let
  cfg = config.services.aquarium-monitor;
  collectorService = "aquarium-monitor.service";
  podmanNetwork = "aquarium-monitor";
  grafanaDatasourceProvisioning = pkgs.writeText "aquarium-monitor-grafana-datasource.yml" ''
    apiVersion: 1

    datasources:
      - name: Aquarium InfluxDB
        uid: aquarium-influxdb
        type: influxdb
        access: proxy
        url: "http://influxdb:${toString cfg.influxdb.port}"
        jsonData:
          version: Flux
          organization: ${cfg.influxdb.organization}
          defaultBucket: ${cfg.influxdb.bucket}
          tlsSkipVerify: false
        secureJsonData:
          token: ''${INFLUXDB_TOKEN}
  '';
  grafanaDashboardProvider = pkgs.writeText "aquarium-monitor-grafana-dashboard-provider.yml" ''
    apiVersion: 1

    providers:
      - name: Aquarium Monitor
        type: file
        disableDeletion: true
        editable: false
        options:
          path: /etc/grafana/provisioning/dashboards
  '';
  grafanaDashboard = pkgs.writeText "aquarium-monitor-grafana-dashboard.json" ''
    {
      "id": null,
      "uid": "aquarium-monitor",
      "title": "Aquarium Monitor",
      "tags": ["aquarium", "telemetry"],
      "timezone": "browser",
      "schemaVersion": 39,
      "version": 1,
      "refresh": "30s",
      "time": {"from": "now-24h", "to": "now"},
      "templating": {
        "list": [
          {
            "name": "tank_id",
            "label": "Tank",
            "type": "query",
            "datasource": {"type": "influxdb", "uid": "aquarium-influxdb"},
            "query": "import \"influxdata/influxdb/schema\"\nschema.tagValues(bucket: v.defaultBucket, tag: \"tank_id\", predicate: (r) => r._measurement == \"aquarium_telemetry\")",
            "refresh": 1,
            "includeAll": true,
            "multi": true,
            "allValue": ".*",
            "current": {"selected": true, "text": "All", "value": ["$__all"]},
            "sort": 1
          }
        ]
      },
      "panels": [
        {
          "id": 1,
          "type": "timeseries",
          "title": "Electrical conductivity",
          "description": "Mean EC in microsiemens per centimetre.",
          "gridPos": {"h": 8, "w": 12, "x": 0, "y": 0},
          "fieldConfig": {"defaults": {"unit": "us", "color": {"mode": "palette-classic"}}, "overrides": []},
          "targets": [{"refId": "A", "queryType": "0", "datasource": {"type": "influxdb", "uid": "aquarium-influxdb"}, "query": "from(bucket: v.defaultBucket)\n  |> range(start: v.timeRangeStart, stop: v.timeRangeStop)\n  |> filter(fn: (r) => r._measurement == \"aquarium_telemetry\")\n  |> filter(fn: (r) => r._field == \"ec_us_cm\")\n  |> filter(fn: (r) => r.tank_id =~ /^''${tank_id:regex}$/)\n  |> aggregateWindow(every: v.windowPeriod, fn: mean, createEmpty: false)\n  |> yield(name: \"mean\")"}],
          "options": {"legend": {"displayMode": "list", "placement": "bottom"}, "tooltip": {"mode": "multi", "sort": "desc"}}
        },
        {
          "id": 2,
          "type": "timeseries",
          "title": "Temperature",
          "gridPos": {"h": 8, "w": 12, "x": 12, "y": 0},
          "fieldConfig": {"defaults": {"unit": "celsius", "color": {"mode": "palette-classic"}}, "overrides": []},
          "targets": [{"refId": "A", "queryType": "0", "datasource": {"type": "influxdb", "uid": "aquarium-influxdb"}, "query": "from(bucket: v.defaultBucket)\n  |> range(start: v.timeRangeStart, stop: v.timeRangeStop)\n  |> filter(fn: (r) => r._measurement == \"aquarium_telemetry\")\n  |> filter(fn: (r) => r._field == \"temp_c\")\n  |> filter(fn: (r) => r.tank_id =~ /^''${tank_id:regex}$/)\n  |> aggregateWindow(every: v.windowPeriod, fn: mean, createEmpty: false)\n  |> yield(name: \"mean\")"}],
          "options": {"legend": {"displayMode": "list", "placement": "bottom"}, "tooltip": {"mode": "multi", "sort": "desc"}}
        },
        {
          "id": 3,
          "type": "stat",
          "title": "Current EC",
          "gridPos": {"h": 5, "w": 6, "x": 0, "y": 8},
          "fieldConfig": {"defaults": {"unit": "us", "decimals": 1}, "overrides": []},
          "targets": [{"refId": "A", "queryType": "0", "datasource": {"type": "influxdb", "uid": "aquarium-influxdb"}, "query": "from(bucket: v.defaultBucket)\n  |> range(start: -30d)\n  |> filter(fn: (r) => r._measurement == \"aquarium_telemetry\")\n  |> filter(fn: (r) => r._field == \"ec_us_cm\")\n  |> filter(fn: (r) => r.tank_id =~ /^''${tank_id:regex}$/)\n  |> last()"}],
          "options": {"reduceOptions": {"values": false, "calcs": ["lastNotNull"], "fields": ""}, "orientation": "auto", "textMode": "auto", "colorMode": "value", "graphMode": "area", "justifyMode": "auto"}
        },
        {
          "id": 4,
          "type": "stat",
          "title": "Current temperature",
          "gridPos": {"h": 5, "w": 6, "x": 6, "y": 8},
          "fieldConfig": {"defaults": {"unit": "celsius", "decimals": 1}, "overrides": []},
          "targets": [{"refId": "A", "queryType": "0", "datasource": {"type": "influxdb", "uid": "aquarium-influxdb"}, "query": "from(bucket: v.defaultBucket)\n  |> range(start: -30d)\n  |> filter(fn: (r) => r._measurement == \"aquarium_telemetry\")\n  |> filter(fn: (r) => r._field == \"temp_c\")\n  |> filter(fn: (r) => r.tank_id =~ /^''${tank_id:regex}$/)\n  |> last()"}],
          "options": {"reduceOptions": {"values": false, "calcs": ["lastNotNull"], "fields": ""}, "orientation": "auto", "textMode": "auto", "colorMode": "value", "graphMode": "area", "justifyMode": "auto"}
        },
        {
          "id": 5,
          "type": "table",
          "title": "Telemetry freshness",
          "description": "Latest telemetry sample for the selected tank, including age in seconds.",
          "gridPos": {"h": 7, "w": 12, "x": 12, "y": 8},
          "fieldConfig": {"defaults": {}, "overrides": []},
          "targets": [{"refId": "A", "queryType": "0", "datasource": {"type": "influxdb", "uid": "aquarium-influxdb"}, "query": "from(bucket: v.defaultBucket)\n  |> range(start: -30d)\n  |> filter(fn: (r) => r._measurement == \"aquarium_telemetry\")\n  |> filter(fn: (r) => r.tank_id =~ /^''${tank_id:regex}$/)\n  |> group(columns: [\"tank_id\", \"_field\"])\n  |> last()\n  |> keep(columns: [\"_time\", \"_value\", \"_field\", \"tank_id\"])\n  |> map(fn: (r) => ({r with age_seconds: float(v: uint(v: now()) - uint(v: r._time)) / 1000000000.0}))"}],
          "options": {"showHeader": true, "sort": [{"desc": true, "displayName": "_time"}]}
        },
        {
          "id": 6,
          "type": "table",
          "title": "Telemetry source and quality",
          "description": "Sample counts grouped by the source and quality tags for the selected tank.",
          "gridPos": {"h": 7, "w": 12, "x": 0, "y": 13},
          "fieldConfig": {"defaults": {}, "overrides": []},
          "targets": [{"refId": "A", "queryType": "0", "datasource": {"type": "influxdb", "uid": "aquarium-influxdb"}, "query": "from(bucket: v.defaultBucket)\n  |> range(start: v.timeRangeStart, stop: v.timeRangeStop)\n  |> filter(fn: (r) => r._measurement == \"aquarium_telemetry\")\n  |> filter(fn: (r) => r.tank_id =~ /^''${tank_id:regex}$/)\n  |> group(columns: [\"tank_id\", \"source\", \"quality\"])\n  |> count(column: \"_value\")\n  |> rename(columns: {_value: \"sample_count\"})"}],
          "options": {"showHeader": true}
        },
        {
          "id": 7,
          "type": "table",
          "title": "Alarm events",
          "description": "Alarm data is available only when the collector is started with --influx-alarm-output.",
          "gridPos": {"h": 8, "w": 24, "x": 0, "y": 20},
          "fieldConfig": {"defaults": {}, "overrides": []},
          "targets": [{"refId": "A", "queryType": "0", "datasource": {"type": "influxdb", "uid": "aquarium-influxdb"}, "query": "from(bucket: v.defaultBucket)\n  |> range(start: v.timeRangeStart, stop: v.timeRangeStop)\n  |> filter(fn: (r) => r._measurement == \"aquarium_alarm\")\n  |> filter(fn: (r) => r.tank_id =~ /^''${tank_id:regex}$/)\n  |> pivot(rowKey: [\"_time\", \"tank_id\", \"rule_id\", \"severity\"], columnKey: [\"_field\"], valueColumn: \"_value\")\n  |> keep(columns: [\"_time\", \"tank_id\", \"rule_id\", \"severity\", \"reason\", \"observed_value\"])\n  |> sort(columns: [\"_time\"], desc: true)\n  |> limit(n: 100)"}],
          "options": {"showHeader": true, "sort": [{"desc": true, "displayName": "_time"}]}
        }
      ]
    }
  '';
  healthCheckOptions = { enable, command, interval, timeout, retries, startPeriod }:
    lib.optionals enable [
      "--health-cmd=${command}"
      "--health-interval=${interval}"
      "--health-timeout=${timeout}"
      "--health-retries=${toString retries}"
      "--health-start-period=${startPeriod}"
    ];
  simulatorCollectorArgs =
    [ "--raw-log" cfg.simulator.rawLogPath
      "--tank-id" cfg.simulator.tankId
      "--sim-temperature-c" (toString cfg.simulator.temperatureC)
      "--influx-batch-size" (toString cfg.influxBatchSize)
    ]
    ++ lib.optional cfg.influxAlarmOutput "--influx-alarm-output"
    ++ lib.optionals (cfg.alarmOutput != null) [ "--alarm-output" cfg.alarmOutput ]
    ++ lib.optionals (cfg.ecMinimum != null) [ "--ec-min" (toString cfg.ecMinimum) ]
    ++ lib.optionals (cfg.ecMaximum != null) [ "--ec-max" (toString cfg.ecMaximum) ]
    ++ lib.optionals (cfg.ecSeverity != null) [ "--ec-severity" cfg.ecSeverity ]
    ++ lib.optionals (cfg.temperatureMinimum != null) [ "--temperature-min" (toString cfg.temperatureMinimum) ]
    ++ lib.optionals (cfg.temperatureMaximum != null) [ "--temperature-max" (toString cfg.temperatureMaximum) ]
    ++ lib.optionals (cfg.temperatureSeverity != null) [ "--temperature-severity" cfg.temperatureSeverity ]
    ++ lib.optionals (cfg.maxAgeSeconds != null) [ "--max-age-seconds" (toString cfg.maxAgeSeconds) ]
    ++ lib.optionals (cfg.staleSeverity != null) [ "--stale-severity" cfg.staleSeverity ]
    ++ lib.optionals (cfg.spikeAbsolute != null) [ "--spike-absolute" (toString cfg.spikeAbsolute) ]
    ++ lib.optionals (cfg.spikeRelative != null) [ "--spike-relative" (toString cfg.spikeRelative) ]
    ++ lib.optionals (cfg.spikeSeverity != null) [ "--spike-severity" cfg.spikeSeverity ]
    ++ [ "--influx-url" "http://127.0.0.1:${toString cfg.influxdb.port}"
         "--influx-organization" cfg.influxdb.organization
         "--influx-bucket" cfg.influxdb.bucket
         "--influx-token-file" "/run/credentials/aquarium-monitor-simulator.service/influxdb-token"
       ];
in
{
  options.services.aquarium-monitor = {
    enable = lib.mkEnableOption "the Aquarium Monitor collector";

    simulator = {
      enable = lib.mkEnableOption "the continuous Aquarium Monitor simulator";
      package = lib.mkOption {
        type = lib.types.package;
        default =
          let package = simulatorPackage pkgs.system;
          in if package == null then
            throw "services.aquarium-monitor.simulator.package must be set when importing the module directly"
          else package;
        description = "Simulator package to run.";
      };
      tankId = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "Required stable tank identity for simulator telemetry.";
      };
      temperatureC = lib.mkOption {
        type = lib.types.nullOr lib.types.float;
        default = null;
        description = "Required finite, non-negative synthetic temperature in degrees Celsius.";
      };
      intervalSeconds = lib.mkOption {
        type = lib.types.ints.positive;
        default = 1;
        description = "Seconds between simulator frames.";
      };
      rawLogPath = lib.mkOption {
        type = lib.types.path;
        default = "/var/lib/aquarium-monitor/simulator-raw-frames.ndjson";
        description = "Path to the simulator's append-only raw frame NDJSON log.";
      };
    };

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

    alarmOutput = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = ''
        Optional alarm NDJSON destination: a local append-only path or the
        literal stderr (journald). Paths below /var/lib/aquarium-monitor are
        writable with the service's DynamicUser; other paths must be prepared
        with permissions for the transient service user.
      '';
    };

    influxBatchSize = lib.mkOption {
      type = lib.types.ints.positive;
      default = 1;
      description = "Number of telemetry and optional alarm events sent per InfluxDB write.";
    };

    influxAlarmOutput = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Persist rule violations in InfluxDB in addition to optional alarm NDJSON.";
    };

    ecMinimum = lib.mkOption { type = lib.types.nullOr lib.types.float; default = null; description = "Optional EC lower limit."; };
    ecMaximum = lib.mkOption { type = lib.types.nullOr lib.types.float; default = null; description = "Optional EC upper limit."; };
    ecSeverity = lib.mkOption { type = lib.types.nullOr (lib.types.enum [ "warning" "critical" ]); default = null; description = "Severity for configured EC limits and missing EC."; };
    temperatureMinimum = lib.mkOption { type = lib.types.nullOr lib.types.float; default = null; description = "Optional temperature lower limit in degrees Celsius."; };
    temperatureMaximum = lib.mkOption { type = lib.types.nullOr lib.types.float; default = null; description = "Optional temperature upper limit in degrees Celsius."; };
    temperatureSeverity = lib.mkOption { type = lib.types.nullOr (lib.types.enum [ "warning" "critical" ]); default = null; description = "Severity for configured temperature limits and missing temperature."; };
    maxAgeSeconds = lib.mkOption { type = lib.types.nullOr lib.types.ints.positive; default = null; description = "Optional maximum sample age."; };
    staleSeverity = lib.mkOption { type = lib.types.nullOr (lib.types.enum [ "warning" "critical" ]); default = null; description = "Severity for stale samples."; };
    spikeAbsolute = lib.mkOption { type = lib.types.nullOr lib.types.float; default = null; description = "Optional absolute EC/temperature spike limit."; };
    spikeRelative = lib.mkOption { type = lib.types.nullOr lib.types.float; default = null; description = "Optional relative EC/temperature spike limit as a ratio."; };
    spikeSeverity = lib.mkOption { type = lib.types.nullOr (lib.types.enum [ "warning" "critical" ]); default = null; description = "Severity for configured spikes."; };

    baudRate = lib.mkOption {
      type = lib.types.ints.positive;
      default = 9600;
      description = "Serial baud rate. Only 9600 is currently supported.";
    };

    influxdb = {
      enable = lib.mkEnableOption "a local InfluxDB 2.x OCI container";
      image = lib.mkOption { type = lib.types.str; default = "influxdb:2.7"; description = "InfluxDB image and tag."; };
      dataDir = lib.mkOption {
        type = lib.types.str;
        default = "/var/lib/aquarium-monitor/influxdb";
        description = ''
          Persistent InfluxDB data directory. Include this directory in local
          filesystem snapshots or stop the container before copying it; the
          OCI module has no application-consistent backup hook.
        '';
      };
      port = lib.mkOption { type = lib.types.port; default = 8086; description = "Local InfluxDB host port."; };
      organization = lib.mkOption { type = lib.types.str; default = "aquarium"; description = "InfluxDB organization."; };
      bucket = lib.mkOption { type = lib.types.str; default = "telemetry"; description = "InfluxDB bucket."; };
      retention = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = ''
          Optional bucket retention duration passed to InfluxDB during initial
          setup (for example, "30d" or "0s" for infinite retention). This is
          an initialization setting and does not change an already-created bucket.
        '';
      };
      healthCheck = {
        enable = lib.mkOption { type = lib.types.bool; default = true; description = "Enable an InfluxDB container health check."; };
        command = lib.mkOption { type = lib.types.str; default = "influx ping --host http://127.0.0.1:8086"; description = "Command run inside the InfluxDB container."; };
        interval = lib.mkOption { type = lib.types.str; default = "30s"; description = "Health-check interval."; };
        timeout = lib.mkOption { type = lib.types.str; default = "5s"; description = "Health-check timeout."; };
        retries = lib.mkOption { type = lib.types.ints.positive; default = 3; description = "Consecutive failures before the container is unhealthy."; };
        startPeriod = lib.mkOption { type = lib.types.str; default = "30s"; description = "Startup grace period."; };
      };
      environmentFile = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        description = ''
          Host-provided EnvironmentFile for InfluxDB 2.x initialization. It must
          define DOCKER_INFLUXDB_INIT_MODE, DOCKER_INFLUXDB_INIT_USERNAME,
          DOCKER_INFLUXDB_INIT_PASSWORD, DOCKER_INFLUXDB_INIT_ORG,
          DOCKER_INFLUXDB_INIT_BUCKET, and DOCKER_INFLUXDB_INIT_ADMIN_TOKEN.
          Keep the file outside the Nix store; sops-nix can provide its runtime path.
        '';
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
      dataDir = lib.mkOption {
        type = lib.types.str;
        default = "/var/lib/aquarium-monitor/grafana";
        description = ''
          Persistent Grafana data directory. Include this directory in local
          filesystem snapshots; the OCI module has no application-consistent
          backup hook.
        '';
      };
       port = lib.mkOption { type = lib.types.port; default = 3000; description = "Grafana host port."; };
       listenAddress = lib.mkOption { type = lib.types.str; default = "127.0.0.1"; description = "Host address on which Grafana is published."; };
      healthCheck = {
        enable = lib.mkOption { type = lib.types.bool; default = true; description = "Enable a Grafana container health check."; };
        command = lib.mkOption { type = lib.types.str; default = "wget --spider --quiet http://127.0.0.1:3000/api/health"; description = "Command run inside the Grafana container."; };
        interval = lib.mkOption { type = lib.types.str; default = "30s"; description = "Health-check interval."; };
        timeout = lib.mkOption { type = lib.types.str; default = "5s"; description = "Health-check timeout."; };
        retries = lib.mkOption { type = lib.types.ints.positive; default = 3; description = "Consecutive failures before the container is unhealthy."; };
        startPeriod = lib.mkOption { type = lib.types.str; default = "30s"; description = "Startup grace period."; };
      };
      provisioning = {
        enable = lib.mkEnableOption "an InfluxDB datasource provisioned in Grafana";
        tokenEnvironmentFile = lib.mkOption {
          type = lib.types.nullOr lib.types.path;
          default = null;
          description = ''
            Host-provided EnvironmentFile containing INFLUXDB_TOKEN for Grafana
            datasource provisioning. Keep this file outside the Nix store; sops-nix
            can provide its runtime path.
          '';
        };
      };
      dashboard = {
        enable = lib.mkEnableOption "a generic Aquarium Monitor Grafana dashboard";
      };
    };
  };

  config = lib.mkMerge [
    {
      assertions = [
        {
          assertion = !(cfg.enable && cfg.simulator.enable);
          message = "services.aquarium-monitor.enable and services.aquarium-monitor.simulator.enable cannot both be enabled";
        }
        {
          assertion = cfg.grafana.listenAddress != "";
          message = "services.aquarium-monitor.grafana.listenAddress must not be empty";
        }
      ];
    }
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
       {
         assertion = cfg.influxdb.retention == null || cfg.influxdb.retention != "";
         message = "InfluxDB retention must not be empty when configured";
       }
       {
         assertion = !cfg.influxdb.healthCheck.enable || cfg.influxdb.healthCheck.command != "";
         message = "InfluxDB health-check command must not be empty when health checks are enabled";
       }
      ];
      virtualisation.oci-containers.containers.influxdb = {
        image = cfg.influxdb.image;
        networks = [ podmanNetwork ];
        ports = [ "127.0.0.1:${toString cfg.influxdb.port}:8086" ];
        volumes = [ "${cfg.influxdb.dataDir}:/var/lib/influxdb2" ];
        environment = lib.optionalAttrs (cfg.influxdb.retention != null) {
          DOCKER_INFLUXDB_INIT_RETENTION = cfg.influxdb.retention;
        };
        environmentFiles = lib.optional (cfg.influxdb.environmentFile != null) cfg.influxdb.environmentFile;
        extraOptions = [ "--pull=missing" "--restart=on-failure" ]
          ++ healthCheckOptions cfg.influxdb.healthCheck;
      };
      systemd.tmpfiles.rules = [
        "d '${cfg.influxdb.dataDir}' 0750 1000 1000 -"
        "Z '${cfg.influxdb.dataDir}' 0750 1000 1000 -"
      ];
      systemd.services.aquarium-monitor-podman-network = {
        description = "Aquarium Monitor Podman network";
        wantedBy = [ "multi-user.target" ];
        before = [ "podman-influxdb.service" "podman-grafana.service" ];
        path = [ pkgs.podman ];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        script = ''
          podman network exists ${podmanNetwork} || podman network create ${podmanNetwork}
        '';
      };
      systemd.services.podman-influxdb.requires = [ "aquarium-monitor-podman-network.service" ];
      systemd.services.podman-influxdb.after = [ "aquarium-monitor-podman-network.service" ];
    })
    (lib.mkIf cfg.grafana.enable {
      assertions = lib.optionals cfg.grafana.provisioning.enable [
        {
          assertion = cfg.influxdb.enable;
          message = "services.aquarium-monitor.grafana.provisioning.enable requires InfluxDB to be enabled";
        }
        {
          assertion = cfg.grafana.provisioning.tokenEnvironmentFile != null;
          message = "services.aquarium-monitor.grafana.provisioning.tokenEnvironmentFile must be set when datasource provisioning is enabled";
        }
      ] ++ [
        {
          assertion = !cfg.grafana.healthCheck.enable || cfg.grafana.healthCheck.command != "";
          message = "Grafana health-check command must not be empty when health checks are enabled";
        }
      ];
      virtualisation.oci-containers.containers.grafana = {
        image = cfg.grafana.image;
        networks = [ podmanNetwork ];
         ports = [ "${cfg.grafana.listenAddress}:${toString cfg.grafana.port}:3000" ];
        volumes = [ "${cfg.grafana.dataDir}:/var/lib/grafana" ]
          ++ lib.optional cfg.grafana.provisioning.enable
            "${grafanaDatasourceProvisioning}:/etc/grafana/provisioning/datasources/aquarium-monitor.yml:ro"
          ++ lib.optionals cfg.grafana.dashboard.enable [
            "${grafanaDashboardProvider}:/etc/grafana/provisioning/dashboards/aquarium-monitor.yml:ro"
            "${grafanaDashboard}:/etc/grafana/provisioning/dashboards/aquarium-monitor.json:ro"
          ];
        environmentFiles = lib.optional cfg.grafana.provisioning.enable cfg.grafana.provisioning.tokenEnvironmentFile;
        dependsOn = lib.optional cfg.influxdb.enable "influxdb";
        extraOptions = [ "--pull=missing" "--restart=on-failure" ]
          ++ healthCheckOptions cfg.grafana.healthCheck;
      };
      systemd.tmpfiles.rules = [
        "d '${cfg.grafana.dataDir}' 0750 472 472 -"
        "Z '${cfg.grafana.dataDir}' 0750 472 472 -"
      ];
      systemd.services.podman-grafana.requires = [ "aquarium-monitor-podman-network.service" ];
      systemd.services.podman-grafana.after = [ "aquarium-monitor-podman-network.service" ];
    })
    (lib.mkIf cfg.grafana.dashboard.enable {
      assertions = [
        {
          assertion = cfg.grafana.enable;
          message = "services.aquarium-monitor.grafana.dashboard.enable requires Grafana to be enabled";
        }
        {
          assertion = cfg.influxdb.enable;
          message = "services.aquarium-monitor.grafana.dashboard.enable requires InfluxDB to be enabled";
        }
        {
          assertion = cfg.grafana.provisioning.enable;
          message = "services.aquarium-monitor.grafana.dashboard.enable requires Grafana datasource provisioning to be enabled";
        }
      ];
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
      } ++ lib.optional cfg.influxAlarmOutput {
        assertion = cfg.influxdb.enable;
        message = "services.aquarium-monitor.influxAlarmOutput requires InfluxDB to be enabled";
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
           ] ++ [ "--influx-batch-size" (toString cfg.influxBatchSize) ] ++ lib.optional cfg.influxAlarmOutput "--influx-alarm-output" ++ lib.optionals (cfg.alarmOutput != null) [ "--alarm-output" cfg.alarmOutput ] ++ lib.optionals (cfg.ecMinimum != null) [ "--ec-min" (toString cfg.ecMinimum) ] ++ lib.optionals (cfg.ecMaximum != null) [ "--ec-max" (toString cfg.ecMaximum) ] ++ lib.optionals (cfg.ecSeverity != null) [ "--ec-severity" cfg.ecSeverity ] ++ lib.optionals (cfg.temperatureMinimum != null) [ "--temperature-min" (toString cfg.temperatureMinimum) ] ++ lib.optionals (cfg.temperatureMaximum != null) [ "--temperature-max" (toString cfg.temperatureMaximum) ] ++ lib.optionals (cfg.temperatureSeverity != null) [ "--temperature-severity" cfg.temperatureSeverity ] ++ lib.optionals (cfg.maxAgeSeconds != null) [ "--max-age-seconds" (toString cfg.maxAgeSeconds) ] ++ lib.optionals (cfg.staleSeverity != null) [ "--stale-severity" cfg.staleSeverity ] ++ lib.optionals (cfg.spikeAbsolute != null) [ "--spike-absolute" (toString cfg.spikeAbsolute) ] ++ lib.optionals (cfg.spikeRelative != null) [ "--spike-relative" (toString cfg.spikeRelative) ] ++ lib.optionals (cfg.spikeSeverity != null) [ "--spike-severity" cfg.spikeSeverity
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
          StateDirectoryMode = "0750";
          DynamicUser = true;
          NoNewPrivileges = true;
          PrivateTmp = true;
          ProtectHome = true;
          ProtectSystem = "strict";
        };
      };
    })
    (lib.mkIf cfg.simulator.enable {
      assertions = [
        { assertion = cfg.simulator.tankId != null && cfg.simulator.tankId != ""; message = "services.aquarium-monitor.simulator.tankId must be set"; }
        { assertion = cfg.simulator.temperatureC != null && cfg.simulator.temperatureC >= 0.0; message = "services.aquarium-monitor.simulator.temperatureC must be finite and non-negative"; }
        { assertion = cfg.influxdb.enable; message = "services.aquarium-monitor.simulator.enable requires InfluxDB to be enabled"; }
        { assertion = cfg.influxdb.environmentFile != null; message = "services.aquarium-monitor.influxdb.environmentFile must be set for the simulator"; }
        { assertion = cfg.influxdb.tokenFile != null; message = "services.aquarium-monitor.influxdb.tokenFile must be set for the simulator"; }
      ];
      systemd.services.aquarium-monitor-simulator = let
        simulatorCommand = lib.escapeShellArgs [
          "${cfg.simulator.package}/bin/simulator-ezo-ec"
          "--continuous"
          "--interval-seconds" (toString cfg.simulator.intervalSeconds)
        ];
        collectorCommand = lib.escapeShellArgs ([ "${cfg.package}/bin/collector" ] ++ simulatorCollectorArgs);
        pipeline = pkgs.writeShellScript "aquarium-monitor-simulator" ''
          set -o pipefail
          ${simulatorCommand} | ${collectorCommand}
        '';
      in {
        description = "Aquarium Monitor continuous simulator";
        wantedBy = [ "multi-user.target" ];
        wants = [ "podman-influxdb.service" ];
        requires = [ "podman-influxdb.service" ];
        after = [ "local-fs.target" "podman-influxdb.service" ];
        serviceConfig = {
          ExecStart = pipeline;
          LoadCredential = [ "influxdb-token:${cfg.influxdb.tokenFile}" ];
          Restart = "on-failure";
          RestartSec = 5;
          StateDirectory = "aquarium-monitor";
          StateDirectoryMode = "0750";
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
