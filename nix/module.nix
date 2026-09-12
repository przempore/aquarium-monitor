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
  };

  config = lib.mkIf cfg.enable {
    systemd.services.aquarium-monitor = {
      description = "Aquarium Monitor collector";
      wantedBy = [ "multi-user.target" ];
      after = [ "local-fs.target" ];

      serviceConfig = {
        ExecStart = lib.escapeShellArgs [
          "${cfg.package}/bin/collector"
          "--raw-log"
          cfg.rawLogPath
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
