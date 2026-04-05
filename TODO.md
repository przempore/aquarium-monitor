# Transport-Agnostic Sensor Emulator Plan

Goal: build the emulator so switching from TCP simulator to USB serial sensor requires no protocol reimplementation.

## Target architectur

- One protocol core (`EzoEcCore`) with no Tokio/TCP/serial dependencies.
- One generic async session loop over `AsyncRead + AsyncWrite`.
- Thin transport adapters:
  - TCP adapter for simulator mode.
  - Serial adapter for real hardware mode.
- Collector depends on a stable `Source` trait, not transport details.

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

## Chapter 2: Build generic session loop

- Implement one async loop operating on generic stream type.
- Signature idea:

```rust
async fn run_session<S>(stream: S, core: EzoEcCore) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
```

- Centralize frame handling (`\r` and `\r\n` normalization).
- Add integration-style test with `tokio::io::duplex`.

Checkpoint:

- Session logic works without TCP-specific code.

## Chapter 3: Add TCP adapter (simulator mode)

- Keep TCP code thin: bind/accept, then call `run_session`.
- Start with per-connection state.
- Keep behavior compatible with current manual checks.

Checkpoint:

- `nc`-based smoke tests work as expected.

## Chapter 4: Add serial adapter (real sensor mode)

- Add `tokio-serial` and open `/dev/ttyUSB*`.
- Pass serial stream to `run_session`.
- Do not duplicate protocol logic.

Checkpoint:

- Same protocol behavior is exercised through serial path.

## Chapter 5: Stable collector interface

- Define stable trait boundary for collector interactions.
- Implement `TcpSource` and `SerialSource` behind same trait.
- Make source selectable via configuration only.

Checkpoint:

- Collector can switch source without protocol code changes.

## Verification flow (after each chapter)

1. `cargo check`
2. `cargo test -p simulator-ezo-ec`
3. Manual smoke checks (`nc` for TCP path)
4. Regenerate `Cargo.nix` when dependencies change

## Defaults for now

- Use per-connection state first.
- Keep parser explicit and strict.
- Use `anyhow` at app boundary; consider typed protocol errors later.
