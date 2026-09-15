# Target Deployment

This deployment runs the simulator, InfluxDB, and Grafana on a NixOS host.
The host must already have SSH and Tailscale configured. Copy the settings from
`target-example.nix` into the host configuration and replace the example
Tailscale address with the output of:

```sh
tailscale ip -4
```

The host flake should import the project module:

```nix
inputs.aquarium-monitor.nixosModules.default
```

It must also provide these sops-nix secrets:

- `aquarium-monitor/influxdb-init`: an EnvironmentFile containing the six
  `DOCKER_INFLUXDB_INIT_*` variables documented in the root README.
- `aquarium-monitor/influxdb-token`: the InfluxDB API token as a file.
- `aquarium-monitor/grafana-token-environment`: an EnvironmentFile containing
  `INFLUXDB_TOKEN` with the same token.

After adding the module to the target host, deploy from the project checkout:

```sh
sudo nixos-rebuild switch --flake .#TARGET --target-host USER@HOST
```

If the host flake is maintained separately, update it there and run the same
command from that checkout. Check the services over SSH:

```sh
ssh USER@HOST systemctl status aquarium-monitor-simulator.service
ssh USER@HOST systemctl status podman-influxdb.service podman-grafana.service
ssh USER@HOST journalctl -u aquarium-monitor-simulator.service -f
```

Open Grafana from an iPhone or Windows machine connected to the same Tailnet:

```text
http://TAILSCALE-IP:3000
```

Grafana is intentionally bound to the configured Tailscale address. InfluxDB
remains bound to `127.0.0.1:8086` and is not exposed remotely. SSH forwarding
is an alternative when changing the firewall is undesirable:

```sh
ssh -L 3000:127.0.0.1:3000 USER@HOST
```

Then browse to `http://127.0.0.1:3000`.

The simulator writes deterministic EC and synthetic temperature data. Its raw
frames and InfluxDB data persist across service restarts. The InfluxDB data
directory should be included in the host's normal backup/snapshot policy.
