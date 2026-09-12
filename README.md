# Aquarium Monitor

Self-hosted aquarium monitoring system focused on **data ownership**, **reproducibility**, and **extensibility**.

The system continuously collects water quality telemetry (EC, temperature; later pH),
stores it locally, visualizes it with Grafana, and provides a foundation for rule-based
and AI-assisted interpretation.

This project is designed to run entirely on a dedicated laptop using:
- **Rust** (collector, simulator, future UI)
- **InfluxDB** (time-series storage)
- **Grafana** (dashboards & alerting)
- **Nix / NixOS** (packaging, deployment, systemd integration)

Hardware planned for the initial build:
- [Atlas Scientific EZO Complete EC (conductivity) kit](https://eu.robotshop.com/products/atlas-scientific-ezo-complete-ec-conductivity-m)
- [DS18B20 waterproof digital temperature sensor kit](https://eu.robotshop.com/products/ds18b20-waterproof-digital-temperature-sensor-kit?qd=b670c402da420233b4881e44f5888760)
- [Atlas Scientific industrial conductivity calibration kit (0.1L set)](https://eu.robotshop.com/products/atlas-scientific-industrial-conductivity-calibration-k-01l-set?qd=f721a0431afb68219b7f4018c5bc046c)
- [ESP32 Thing development board](https://eu.robotshop.com/products/esp32-thing-development-board?qd=418e6102bc9664eca383a8088570c8ae)

---

## High-level architecture

```text
+---------------------+
|     Sensor Source    |
| - EZO-EC (UART)      |
| - DS18B20            |
| - Simulator (CLI)    |
+----------+-----------+
           |
           | ASCII / CSV (EZO protocol)
           v
+---------------------------+
| Collector (Rust, systemd) |
| - Source adapters         |
| - Parser (EZO protocol)   |
| - Normalizer (units)      |
| - Rule engine (MVP)       |
| - InfluxDB writer         |
| - local raw-frame NDJSON  |
+-------------+-------------+
              |
              | Line protocol (HTTP)
              v
+--------------------+     +------------------+
| InfluxDB (Docker)  |<--->| Grafana (Docker) |
+--------------------+     +------------------+
              |
              | dashboards / alerts
              v
+----------------------+
| Web UI (Dioxus, PWA) |
+----------------------+
```


---

## Core components

### 1. Sensor sources
- **Simulator**: CLI program emulating an Atlas Scientific EZO-EC device
  (stdin/stdout, ASCII protocol).
- **Real hardware**: EZO-EC over UART/USB + DS18B20 (temperature).

Both are abstracted behind a common `Source` interface.

The transport-neutral EZO-EC request/response source contract is implemented.
The Linux w1 DS18B20 source parses standard `w1_slave` files, validates the
`YES` CRC marker, and normalizes `t=` millidegrees Celsius. Hardware mode
combines one valid reading from each sensor into one `TelemetrySample`.
it sends `R\r`, flushes the transport, and reads complete simulator-compatible
responses. A synchronous Linux serial transport opens the configured device;
systemd configures it with `stty` before starting the collector.

---

### 2. Collector (Rust)
The collector supports synchronous stdin batch mode and long-lived Linux device
mode. Device mode requests one frame per interval, logs it before parsing, and
reports source/parse errors on stderr. With explicit InfluxDB options it writes
normalized combined samples to InfluxDB; otherwise it retains normalized NDJSON
on stdout. Stdin mode is unchanged.

Responsibilities:
- Poll sensor sources at a fixed interval
- Parse raw ASCII frames (Atlas EZO protocol)
- Normalize to stable domain units:
  - `ec_us_cm`
  - `temp_c`
- Apply simple rule-based checks (spikes, drift, missing data)
- Persist telemetry to InfluxDB
- Persist every received raw frame, including malformed frames, to a separate
  local NDJSON log for debugging/replay

Collector is intentionally **headless** and independent of any UI.

For local operation, provide the raw log explicitly:

```sh
printf '?R,EC,450.00\n\r*OK\n\r' | cargo run -p collector -- --raw-log raw-frames.ndjson
```

Normalized telemetry is emitted on stdout; raw frames are appended to the
configured file.

The simulator can also provide a live deterministic stream directly to the
collector. It emits one EZO-EC frame immediately and then once per second:

```sh
nix develop --impure --command sh -c \
  'cargo run --quiet -p simulator-ezo-ec -- --continuous | \
   cargo run --quiet -p collector -- --raw-log raw-frames.ndjson'
```

Use `--interval-seconds N` with `--continuous` to choose a positive interval.
The collector still reads stdin until EOF and has no signal handling yet. Stop
the simulator to close the pipe; the collector then finishes after receiving
EOF.

---

### 3. Storage: InfluxDB
- Dedicated time-series database
- Optimized for high-frequency append-only telemetry
- Retention and downsampling handled natively
- Queried by Grafana

The synchronous InfluxDB 2.x sink is selected by hardware mode and requires
explicit `url`, `organization`, `bucket`, and token-file configuration. The
collector reads the token from a protected file at startup, removes only its
final newline, and never logs or passes the token as a command-line argument.
It writes:

```text
aquarium_telemetry tank_id=tank-1 source=hardware quality=ok ec_us_cm=450,temp_c=26.187 1770300600000000000
```

The timestamp is nanoseconds since Unix epoch and requests use
`/api/v2/write?org=...&bucket=...&precision=ns`. The stable `tank_id`, `source`,
and `quality` values are escaped InfluxDB tags.
The standard-library transport is intentionally restricted to local
`http://` networking for now. HTTPS/TLS is not supported yet. Tokens are not
included in displayed sink errors.

InfluxDB is the **single source of truth** for measurements.

---

### 4. Visualization & alerting: Grafana
- Dashboards for EC and temperature
- Threshold and trend-based alerting
- Alerts work even when no UI is open

Grafana does **not** store measurement history itself.

---

### 5. Web UI (planned)
- Built with **Dioxus**
- Serves as:
  - control surface (pause/resume polling, annotate water changes)
  - visualization wrapper around Grafana
- Delivered as **PWA** for desktop and mobile

Push notifications (iOS/Android) will be driven by backend alerts,
not by the UI being open.

---

## Data model

### Domain telemetry (normalized)

```json
{
  "tank_id": "tank-1",
  "timestamp": "2026-02-05T15:10:00Z",
  "ec_us_cm": 132.4,
  "temp_c": 26.4,
  "source": "sim",
  "quality": "ok"
}
```

Only normalized data is written to the normalized output. Raw sensor frames are
logged separately as NDJSON records containing `timestamp` and `raw_frame`.
The collector accepts the explicit configuration `--raw-log PATH`; normalized
NDJSON remains on stdout.

Device mode requires `--tank-id ID`, `--device PATH`, `--temperature-path PATH`,
and `--interval-seconds N`:

```sh
nix develop --impure --command cargo run --quiet -p collector -- \
  --device /dev/ttyUSB0 --temperature-path /sys/bus/w1/devices/28-000000000000/w1_slave \
  --tank-id tank-1 --interval-seconds 1 --raw-log /tmp/ezo-ec-raw.ndjson
```

---

## Deployment model

### NixOS / systemd and OCI containers

- The collector runs as a native systemd service.
- Optional InfluxDB and Grafana containers are managed by
  `virtualisation.oci-containers` (Podman), not Docker Compose.
- InfluxDB is bound to `127.0.0.1:8086`; Grafana is bound to
  `127.0.0.1:3000`.
- Container data is persisted under `/var/lib/aquarium-monitor/` by default.

Create `/etc/aquarium-monitor/influxdb.env` and
`/run/keys/aquarium-monitor-influxdb-token` outside the repository with
restrictive permissions. The environment file must contain InfluxDB's
`DOCKER_INFLUXDB_INIT_*` values, including `DOCKER_INFLUXDB_INIT_ADMIN_TOKEN`;
the token file contains that same token on one line.

For example, the environment file can contain:

```text
DOCKER_INFLUXDB_INIT_MODE=setup
DOCKER_INFLUXDB_INIT_USERNAME=aquarium
DOCKER_INFLUXDB_INIT_PASSWORD=use-a-local-password
DOCKER_INFLUXDB_INIT_ORG=aquarium
DOCKER_INFLUXDB_INIT_BUCKET=telemetry
DOCKER_INFLUXDB_INIT_ADMIN_TOKEN=replace-with-a-long-random-token
```

### NixOS / systemd

- Collector installed as a Nix package
- Service managed by systemd

The flake exports a reusable NixOS module. Import it and enable the service in
your host configuration:

```nix
{
  imports = [ inputs.aquarium-monitor.nixosModules.default ];

  services.aquarium-monitor.enable = true;
  services.aquarium-monitor.device = "/dev/ttyUSB0";
  services.aquarium-monitor.tankId = "tank-1";
  services.aquarium-monitor.temperaturePath = "/sys/bus/w1/devices/28-000000000000/w1_slave";
  services.aquarium-monitor.intervalSeconds = 1;
  services.aquarium-monitor.influxdb = {
    enable = true;
    environmentFile = config.sops.secrets."aquarium-monitor/influxdb-init".path;
    tokenFile = config.sops.secrets."aquarium-monitor/influxdb-token".path;
    organization = "aquarium";
    bucket = "telemetry";
  };
  services.aquarium-monitor.grafana.enable = true;
}
```

The example assumes `sops-nix` is already enabled in the host configuration:

```nix
sops.secrets."aquarium-monitor/influxdb-init" = { mode = "0400"; };
sops.secrets."aquarium-monitor/influxdb-token" = { mode = "0400"; };
```

The repository module remains secret-provider agnostic; `sops-nix` supplies the
paths and the collector receives the token through a systemd credential.

The module persists raw frames at
`/var/lib/aquarium-monitor/raw-frames.ndjson` by default, starts the collector
at boot, and restarts it on failure. `services.aquarium-monitor.package` can
override the flake's `packages.collector` default. The service uses `stty` in
`ExecStartPre` and currently supports only 9600 baud. Configure host
udev/group policy so the service's `DynamicUser` can open the device.

The service uses `/dev/null` as stdin and runs the long-lived device collector.
When InfluxDB is enabled, it requires the local container and connects to
`http://127.0.0.1:8086`.
Arbitrary baud rates are intentionally rejected until serial configuration is
validated for this transport.

Hardware mode requires `tankId`, `device`, and `temperaturePath`; the module
passes all three explicitly to the collector. `tankId` has no default because
the reusable module is not inherently limited to one tank. Enable the Linux kernel modules with
`boot.kernelModules = [ "w1-gpio" "w1-therm" ];`. Physical hardware and the
OCI container setup have not been tested yet. Validate serial permissions, w1
paths, Podman storage, image pulls, and secret file ownership on the host.

---

## Project goals

- Deterministic, inspectable data flow
- No cloud dependencies
- Hardware-swappable architecture
- Strong separation of concerns
- Long-term maintainability

---

## Status

- [x] Architecture defined
- [x] Sensor simulator (EZO-EC emulator)
- [x] Collector MVP (stdin framing, parsing, and NDJSON output)
- [x] Local raw-frame NDJSON logging, including malformed frames, with
      `--raw-log PATH` collector configuration
- [x] Continuous deterministic simulator mode with configurable interval
- [x] Transport-neutral EZO-EC request/response source contract
- [x] Linux serial transport, bounded polling API, and long-lived physical source mode
- [x] DS18B20 Linux w1 parser/source, combined samples, and NixOS path configuration
- [ ] Validate EZO-EC and DS18B20 integration on physical hardware
- [x] InfluxDB 2.x device-mode sink with protected token-file configuration
- [x] Optional NixOS-managed InfluxDB and Grafana OCI containers
- [ ] InfluxDB + Grafana live integration on target hardware
- [ ] Rule-based alerting
- [ ] Web UI (Dioxus)
- [ ] AI-assisted interpretation layer
