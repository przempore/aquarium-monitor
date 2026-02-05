# AGENTS.md

This file provides guidance for AI agents (Codex, ChatGPT, etc.)
working on this repository.

---

## Project intent

This project is a **self-hosted aquarium monitoring system**.

Key principles:
- No cloud dependencies
- All telemetry stored locally
- Deterministic data pipeline
- Rust-first (including embedded later)
- Nix/NixOS-native deployment

Agents should prioritize:
- correctness over cleverness
- explicit data models
- debuggability
- composable architecture

---

## Technology stack (authoritative)

- Language: **Rust**
- OS / deployment: **NixOS**, flakes
- Service management: **systemd**
- Time-series DB: **InfluxDB**
- Visualization: **Grafana**
- UI (planned): **Dioxus**
- Embedded (planned): **Rust on ESP32**

Do **not** introduce:
- cloud services
- proprietary monitoring stacks
- unnecessary message brokers
- relational databases unless explicitly requested

---

## Architecture rules

1. **Collector is the brain**
   - All interpretation, normalization, and alarms must work without UI.

2. **UI is optional**
   - No logic should depend on the UI being open.

3. **Raw vs normalized data**
   - Raw sensor frames:
     - logged only (journald or NDJSON)
   - Normalized telemetry:
     - written to InfluxDB

4. **One stable domain model**
   - Internals may change
   - Domain telemetry must remain stable

5. **Hardware is a plugin**
   - Simulator and real sensors must share the same interface.

---

## Expected code structure

```text
aquarium-monitor/
├── flake.nix
├── Cargo.toml              # workspace
├── docs/                    # sensor + water parameter docs
├── crates/
│   ├── collector/
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   ├── simulator-ezo-ec/
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   └── common/
│       ├── Cargo.toml
│       └── src/lib.rs
├── nix/
│   └── module.nix
└── deploy/
    └── docker-compose.yml
```


Agents should respect this separation.

---

## Coding guidelines

- Prefer explicit types over magic numbers
- Avoid global state
- No blocking I/O in async contexts
- Log raw data sparingly, but never silently discard it
- Errors should be observable (logs + metrics)

---

## How to extend the project

Safe extensions:
- additional sensors (pH, ORP)
- more rules (trend analysis)
- AI-generated reports
- multi-tank support

Extensions must:
- preserve existing data formats
- not require rewriting the pipeline

---

## Non-goals

- Cloud dashboards
- Social features
- Remote SaaS dependencies
- "Smart" behavior without explainability

---

## When in doubt

Prefer:
- simpler architecture
- clearer boundaries
- less magic

This is a **monitoring system**, not a demo.
