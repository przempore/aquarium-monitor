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
mode. Device mode requests one frame per interval, logs it before parsing,
reports source/parse errors on stderr, and emits normalized NDJSON on stdout.

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

Device mode requires `--device PATH`, `--temperature-path PATH`, and
`--interval-seconds N`:

```sh
nix develop --impure --command cargo run --quiet -p collector -- \
  --device /dev/ttyUSB0 --temperature-path /sys/bus/w1/devices/28-000000000000/w1_slave \
  --interval-seconds 1 --raw-log /tmp/ezo-ec-raw.ndjson
```

---

## Deployment model

### Docker

- InfluxDB
- Grafana

Managed via `docker-compose`.

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
  services.aquarium-monitor.temperaturePath = "/sys/bus/w1/devices/28-000000000000/w1_slave";
  services.aquarium-monitor.intervalSeconds = 1;
  # services.aquarium-monitor.rawLogPath = "/var/lib/aquarium-monitor/raw-frames.ndjson";
}
```

The module persists raw frames at
`/var/lib/aquarium-monitor/raw-frames.ndjson` by default, starts the collector
at boot, and restarts it on failure. `services.aquarium-monitor.package` can
override the flake's `packages.collector` default. The service uses `stty` in
`ExecStartPre` and currently supports only 9600 baud. Configure host
udev/group policy so the service's `DynamicUser` can open the device.

The service uses `/dev/null` as stdin and runs the long-lived device collector.
Arbitrary baud rates are intentionally rejected until serial configuration is
validated for this transport.

Hardware mode requires both `device` and `temperaturePath`; the module passes
both paths explicitly to the collector. Enable the Linux kernel modules with
`boot.kernelModules = [ "w1-gpio" "w1-therm" ];`. Physical hardware has not
been tested yet.

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
- [ ] InfluxDB + Grafana integration
- [ ] Rule-based alerting
- [ ] Web UI (Dioxus)
- [ ] AI-assisted interpretation layer
