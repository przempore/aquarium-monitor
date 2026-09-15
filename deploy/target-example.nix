# Host configuration fragment for a software-only Aquarium Monitor deployment.
# Import this file from the target machine's NixOS configuration, and import
# `inputs.aquarium-monitor.nixosModules.default` from that machine's flake.
{ config, ... }:
{
  services.tailscale.enable = true;

  # Keep Grafana private to the Tailscale interface. Replace this with the
  # address shown by `tailscale ip -4` on the target host.
  networking.firewall.interfaces.tailscale0.allowedTCPPorts = [ 3000 ];

  services.aquarium-monitor = {
    simulator = {
      enable = true;
      tankId = "demo-tank";
      temperatureC = 26.5;
      intervalSeconds = 1;
      rawLogPath = "/var/lib/aquarium-monitor/simulator-raw-frames.ndjson";
    };

    influxdb = {
      enable = true;
      organization = "aquarium";
      bucket = "telemetry";
      retention = "30d";
      environmentFile = config.sops.secrets."aquarium-monitor/influxdb-init".path;
      tokenFile = config.sops.secrets."aquarium-monitor/influxdb-token".path;
    };

    influxBatchSize = 10;
    influxAlarmOutput = true;
    alarmOutput = "stderr";

    # These are demonstration limits only. Replace them with aquarium-specific
    # values after real sensor calibration.
    temperatureMinimum = 20.0;
    temperatureMaximum = 30.0;
    temperatureSeverity = "warning";

    grafana = {
      enable = true;
      listenAddress = "100.64.0.10";
      provisioning = {
        enable = true;
        tokenEnvironmentFile = config.sops.secrets."aquarium-monitor/grafana-token-environment".path;
      };
      dashboard.enable = true;
    };
  };
}
