# Progress

## Project intent

Aquarium Monitor is a self-hosted, local-first system for collecting and
normalizing aquarium telemetry. The design favors deterministic processing,
replaceable sensor sources, and an optional UI.

## Completed milestones

- Shared `TelemetrySample` model with timestamp, EC, temperature, source, and
  quality fields.
- Strict EZO-EC parser for valid response shape, measurement type, numeric
  finite non-negative values, and optional `*OK` status.
- Deterministic, stateful EZO-EC simulator with command handling and changing
  conductivity readings.
- Synchronous `Source` and `Sink` traits for sensor input and normalized output.
- Transport-neutral `Source` and `Sink` contracts now live in `common`; collector
  re-exports them for its existing API while adapters can depend on `common`
  without depending on collector.
- Bounded polling with configurable attempts and interval, including
  deterministic injected scheduling for tests.
- Stdin-to-NDJSON collector path: complete EZO frames are read from stdin,
  normalized samples are written one per line to stdout, and malformed frames
  are reported without stopping later valid frames.
- Local raw-frame NDJSON logging: every received frame is written with a UTC
  timestamp and its exact raw contents before parsing, including malformed or
  rejected frames.
- Minimal collector configuration: `--raw-log PATH` selects an append-only
  raw log while normalized NDJSON remains on stdout.
- Continuous simulator mode: `--continuous` emits deterministic EZO-EC frames
  immediately and at a configurable positive `--interval-seconds N` interval.
- NixOS deployment module: the flake exports `nixosModules.default`, with the
  collector package exposed as `packages.collector`, a persistent raw log, and
  a hardened systemd service definition.

## Current data flow

```text
EZO-EC simulator or stdin
        -> StdinSource
        +-> RawFrameLogger -> local raw NDJSON log
        +-> strict EZO-EC parser
        -> TelemetrySample
        -> NdjsonSink
        -> stdout (one JSON object per line)
```

The current collector executable reads until EOF. It persists raw input when
invoked with `--raw-log PATH`; normalized output remains on stdout. It does not
yet run a long-lived polling service or persist normalized output to a database.
The simulator can run continuously as the collector's stdin; stopping the
simulator closes the pipe, allowing the collector to observe EOF and exit. The
collector has no signal handling yet.

## Explicit limitations

- No InfluxDB storage or InfluxDB boundary.
- No physical sensor or UART/USB hardware source.
- The systemd module currently supplies `/dev/null` as stdin because there is
  no physical source yet. Since the collector exits on stdin EOF, enabling the
  service alone exits immediately and is not a production deployment.
- No rules, alarms, or trend analysis.
- No Grafana dashboards.
- No UI.
- No AI-assisted interpretation.
- Raw frames are persisted locally as NDJSON; rejected frames are also reported
  on stderr.

## Verification

Run the workspace tests:

```sh
nix develop --impure -c cargo test --workspace
```

Exercise the simulator-to-collector path:

```sh
nix develop --impure --command sh -c 'printf "R\nR\n" | cargo run --quiet -p simulator-ezo-ec | cargo run --quiet -p collector -- --raw-log /tmp/aquarium-monitor-raw.ndjson'
```

The second command emits two normalized NDJSON samples and appends both raw
frames to the local raw log.

For the two-process live path:

```sh
nix develop --impure --command sh -c \
  'cargo run --quiet -p simulator-ezo-ec -- --continuous --interval-seconds 2 | \
   cargo run --quiet -p collector -- --raw-log /tmp/aquarium-monitor-live.ndjson'
```

This runs until the simulator is stopped. The collector reads until EOF and
then exits; it has no signal handling yet.

## NixOS module

Import `inputs.aquarium-monitor.nixosModules.default` into a NixOS host and
set `services.aquarium-monitor.enable = true`. The module defaults to the
flake's `packages.collector` and writes raw frames to
`/var/lib/aquarium-monitor/raw-frames.ndjson`; set
`services.aquarium-monitor.rawLogPath` or `package` to override those values.

The module is packaging and service plumbing only. A hardware source or a
long-lived stdin producer must be connected before enabling it in production.

## Next recommended milestone

Implement a physical sensor source against the shared `common::Source` contract
and connect it to the existing synchronous polling boundary. This is the next
appropriate step before adding a normalized telemetry database or deployment
integration.
