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
- No long-running service or systemd integration.
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

## Next recommended milestone

Implement the long-lived polling boundary and a physical sensor source while
keeping the synchronous, local-first pipeline. This is the next appropriate
step before adding a normalized telemetry database or deployment integration.
