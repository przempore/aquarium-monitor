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
- Transport-neutral EZO-EC request/response source implementing `common::Source`;
  it sends `R\r`, flushes, and reads simulator-compatible framed responses.
- Synchronous DS18B20 Linux w1 source/parser for `w1_slave` content, including
  `YES` CRC validation, millidegree conversion, typed parse/I/O errors, and
  injected-reader tests.
- Combined EZO-EC plus DS18B20 normalization into one `TelemetrySample`.
- Dependency-light synchronous InfluxDB 2.x line-protocol sink with an
  injectable HTTP transport, HTTP-only standard-library client, and tests.

## Current data flow

```text
EZO-EC simulator or stdin
        -> StdinSource
        +-> RawFrameLogger -> local raw NDJSON log
        +-> strict EZO-EC parser
        -> TelemetrySample
         -> InfluxDbSink (hardware mode)
         -> NdjsonSink (stdin/demo mode)
```

Hardware mode reads the EZO-EC serial device and configured DS18B20 `w1_slave`
path, logs both raw payloads, and writes a combined sample to the selected sink.
NixOS hardware mode requires both paths and the host must enable `w1-gpio` and
`w1-therm`.

The request/response source contract is connected to a synchronous Linux serial
transport. The wrapper opens `/dev/tty*` or `/dev/ttyUSB*` read/write; systemd
configures the line with `stty`.

The current collector executable reads until EOF. It persists raw input when
invoked with `--raw-log PATH`; normalized output remains on stdout. It does not
now runs a long-lived synchronous device polling service while still supporting
stdin batch mode. Its bounded, injectable polling API keeps tests finite.
The simulator can run continuously as the collector's stdin; stopping the
simulator closes the pipe, allowing the collector to observe EOF and exit. The
collector has no signal handling yet.

## Explicit limitations

- Device mode writes normalized samples to InfluxDB and does not emit routine
  NDJSON to stdout; stdin/demo mode remains unchanged.
- InfluxDB transport currently supports local `http://` networking only. HTTPS
  and TLS are intentionally deferred.
- No physical hardware or container deployment has been exercised; serial setup is delegated to
  `ExecStartPre` and currently supports only 9600 baud. DS18B20 integration is
  likewise untested against a physical sensor.
- The NixOS service requires `device`, uses `/dev/null` as stdin, and needs host
  udev/group policy to grant its `DynamicUser` access to the serial device.
- No rules, alarms, or trend analysis.
- No Grafana dashboards.
- No UI.
- No AI-assisted interpretation.
- Raw frames are persisted locally as NDJSON; rejected frames are also reported
  on stderr.

## Verification

Run the workspace tests (passing for this milestone):

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
set `services.aquarium-monitor.enable = true`, `device = "/dev/ttyUSB0"`,
`temperaturePath = "/sys/bus/w1/devices/28-.../w1_slave"`, and
`intervalSeconds = 1`. The module defaults to the
flake's `packages.collector` and writes raw frames to
`/var/lib/aquarium-monitor/raw-frames.ndjson`; set
`services.aquarium-monitor.rawLogPath` or `package` to override those values.
Module evaluation checks the generated device, interval, and `stty` command.

The module is packaging and service plumbing only. Hardware access permissions
must be configured by the host.

## InfluxDB sink configuration

Hardware mode requires `--influx-url`, `--influx-organization`,
`--influx-bucket`, and `--influx-token-file`. The token file is read at startup;
its final newline is removed, but other whitespace is preserved. HTTPS is
rejected because the standard-library transport supports local `http://` only.

The sink writes measurement `aquarium_telemetry` with `source` and `quality`
tags, optional `ec_us_cm` and `temp_c` fields, and nanosecond timestamps:

```text
aquarium_telemetry source=hardware quality=ok ec_us_cm=450,temp_c=26.187 1770300600000000000
```

The current model has no tank identifier, so no tank tag is emitted. Tokens are
redacted from configuration debug output and sink error display text.

## NixOS deployment files

The README example requires uncommitted InfluxDB environment and token files.
Do not put credentials or generated container data in Nix source control. The
default local ports are InfluxDB `127.0.0.1:8086` and Grafana `127.0.0.1:3000`;
both containers are disabled unless explicitly enabled. Host validation remains
required for hardware, Podman, directory ownership, and image availability.

## Next recommended milestone

Exercise the serial path and OCI services on the target host, then validate
Grafana dashboards and InfluxDB retention settings.
