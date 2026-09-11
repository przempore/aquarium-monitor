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

## Current data flow

```text
EZO-EC simulator or stdin
        -> StdinSource
        -> strict EZO-EC parser
        -> TelemetrySample
        -> NdjsonSink
        -> stdout (one JSON object per line)
```

The current collector executable reads until EOF. It does not yet run a
long-lived polling service or persist output.

## Explicit limitations

- No InfluxDB storage or InfluxDB boundary.
- No physical sensor or UART/USB hardware source.
- No long-running service or systemd integration.
- No rules, alarms, or trend analysis.
- No Grafana dashboards.
- No UI.
- No AI-assisted interpretation.
- Raw frames are reported on stderr for rejected input, but are not yet
  persisted to a local log.

## Verification

Run the workspace tests:

```sh
nix develop --impure -c cargo test --workspace
```

Exercise the simulator-to-collector path:

```sh
nix develop --impure -c sh -c 'printf "R\nR\n" | cargo run --quiet -p simulator-ezo-ec | cargo run --quiet -p collector'
```

The second command emits two normalized NDJSON samples.

## Next recommended milestone

Implement configurable local raw-frame logging and collector configuration.
This is the most appropriate next boundary because it adds durable,
inspectable operation without introducing an external database before source,
normalization, and error handling are exercised by a long-running process.
