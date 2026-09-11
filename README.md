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
| - journald raw logging    |
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

---

### 2. Collector (Rust)
Runs as a **systemd service** installed via Nix.

Responsibilities:
- Poll sensor sources at a fixed interval
- Parse raw ASCII frames (Atlas EZO protocol)
- Normalize to stable domain units:
  - `ec_us_cm`
  - `temp_c`
- Apply simple rule-based checks (spikes, drift, missing data)
- Persist telemetry to InfluxDB
- Log raw frames to `journald` for debugging/replay

Collector is intentionally **headless** and independent of any UI.

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

Only normalized data is written to InfluxDB.
Raw sensor frames are logged separately.

---

## Deployment model

### Docker

- InfluxDB
- Grafana

Managed via `docker-compose`.

### NixOS / systemd

- Collector installed as a Nix package
- Service managed by systemd
- Requires access to:
  - `journald`
  - `/dev/ttyUSB*`

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
- [x] Collector MVP (strict parser, synchronous polling, and source/sink pipeline)
- [ ] InfluxDB + Grafana integration
- [ ] Rule-based alerting
- [ ] Web UI (Dioxus)
- [ ] AI-assisted interpretation layer
