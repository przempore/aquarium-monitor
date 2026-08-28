# Sensor Emulator Plan

Goal: build a reusable EZO-EC simulator crate with a stable protocol core and async simulator engine.

Update: transport I/O loop for integration is handled outside this crate. This crate provides async behavior (commands + periodic responses) without owning network I/O.

## Target architecture

- One protocol core (`EzoEcCore`) with no Tokio/TCP/serial dependencies.
- One async simulator engine that:
  - accepts commands asynchronously,
  - emits periodic responses/events based on configured interval,
  - keeps deterministic state transitions.
- One standalone CLI mode using stdio (`stdin`/`stdout`) for local interaction.
- This crate is simulator-only.

## Chapter 1: Isolate protocol core

- Move command semantics into pure methods.
- Keep core synchronous and deterministic.
- Create `core.rs` with `EzoEcCore` and `handle_command`.
- Cover commands in unit tests:
  - `i`
  - `C,?`
  - `C,42`
  - invalid command

Checkpoint:

- Core has no Tokio types.
- Unit tests pass for command behavior.

## Chapter 2: Expose crate API for host integration

- Keep protocol core synchronous and transport-agnostic.
- Expose a minimal API that host crates can call directly:
  - `handle_command(&mut self, cmd: &str) -> String`
  - optional command normalization helper.

Checkpoint:

- Host crate can call simulator API directly without transport dependencies.

## Chapter 3: Add async simulator engine (no transport)

- Build an internal async task/actor that owns `EzoEcCore` state.
- Support async command requests while periodic emissions continue.
- Use channels for host integration (for example: command requests in, emitted frames out).

Checkpoint:

- Host crate can send commands asynchronously and receive periodic outputs concurrently.

## Chapter 4: Expand protocol behavior

- Implement missing command semantics required by collector tests.
- Keep responses deterministic and explicitly formatted.
- Add negative-path handling for malformed commands.

Checkpoint:

- Protocol command tests cover expected and invalid paths.

## Chapter 5: Add fixture-style tests for host crates

- Add reusable command/response fixtures (table-driven tests).
- Verify state transitions across command sequences.
- Keep tests independent from network/async runtime.

Checkpoint:

- Host crates can rely on stable simulator behavior via fixtures.

## Chapter 6: Collector integration contract

- Document a stable command/response contract for collector usage.
- Keep library API transport-free.

Checkpoint:

- Collector can use this simulator source without protocol-specific hacks.

## Chapter 7: Standalone stdio CLI mode

- Provide a binary entrypoint that reads commands from `stdin` and writes frames to `stdout`.
- Reuse the same simulator engine/core (no protocol duplication).
- Keep it single-instance and focused on local CLI workflows.

Checkpoint:

- Simulator can be run directly in terminal and controlled over stdio.

## Verification flow (after each chapter)

1. `cargo check`
2. `cargo test -p simulator-ezo-ec`
3. Regenerate `Cargo.nix` when dependencies change

## Defaults for now

- Use per-connection state first.
- Keep parser explicit and strict.
- Use `anyhow` at app boundary; consider typed protocol errors later.
