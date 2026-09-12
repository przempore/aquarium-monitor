{ defaultPackage ? (_: null), lib, pkgs, config, ... }:

let
  cfg = config.services.aquarium-monitor;
in
{
  options.services.aquarium-monitor = {
    enable = lib.mkEnableOption "the Aquarium Monitor collector";

    package = lib.mkOption {
      type = lib.types.package;
      default =
        let
          package = defaultPackage pkgs.system;
        in
        if package == null then
          throw "services.aquarium-monitor.package must be set when importing the module directly"
        else
          package;
      description = "Collector package to run.";
    };

    rawLogPath = lib.mkOption {
      type = lib.types.path;
      default = "/var/lib/aquarium-monitor/raw-frames.ndjson";
      description = ''
        Path to the append-only raw frame NDJSON log. The default is inside
        the service's persistent StateDirectory.
      '';
    };

    device = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = "Linux serial device path for an EZO-EC probe.";
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
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.device != null;
        message = "services.aquarium-monitor.device must be set for the hardware collector service";
      }
      {
        assertion = cfg.baudRate == 9600;
        message = "services.aquarium-monitor.baudRate must be 9600; arbitrary baud rates are not supported yet";
      }
    ];
    systemd.services.aquarium-monitor = {
      description = "Aquarium Monitor collector";
      wantedBy = [ "multi-user.target" ];
      after = [ "local-fs.target" ];

      serviceConfig = {
        ExecStart = lib.escapeShellArgs [
          "${cfg.package}/bin/collector"
          "--raw-log"
          cfg.rawLogPath
          "--device"
          cfg.device
          "--interval-seconds"
          (toString cfg.intervalSeconds)
        ];
        ExecStartPre = lib.escapeShellArgs [
          "${pkgs.coreutils}/bin/stty"
          "-F"
          cfg.device
          "9600"
          "raw"
          "-echo"
          "-ixon"
          "-ixoff"
          "-crtscts"
          "min"
          "1"
          "time"
          "10"
        ];
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
  };
}
