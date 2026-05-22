# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo check                    # Quick type check
cargo build                    # Debug build
cargo build --release          # Release build
cargo test                     # Run all tests
cargo clippy                   # Linter
cargo fmt                      # Formatter
```

Cross-compile for aarch64 (primary deployment target — NXP i.MX93):
```bash
cargo build --release --target aarch64-unknown-linux-gnu
```

Requires `gcc-aarch64-linux-gnu` (`sudo apt install gcc-aarch64-linux-gnu`). The `.cargo/config.toml` automatically sets the linker, `target-cpu=cortex-a55`, and `+neon` for this target.

Run a single test:
```bash
cargo test <test_name>
```

## Gotchas

- **ML inference is aarch64-only.** `src/tss_ml.rs` and `lib/libtss_svm.a` are gated with `#[cfg(target_arch = "aarch64")]`. x86_64 builds compile and run, but `POST /{n}/csv/infer` returns an error.
- **`algorithm.dat` must be present in the working directory at runtime** for ML inference to initialise (`cls_ops.init()` reads it on first call).
- **Config format uses `[[sensors]]` array** (not the old `[modbus1]`/`[modbus2]` style). Defaults load from `config.toml` at startup.
- **`tokio_unstable`** is set globally in `.cargo/config.toml` — do not add it manually to build commands.

## Architecture

See `.claude/CLAUDE.md` for the full request-flow diagram, multi-sensor design, streaming pipeline, CSV recording, and ML inference details.
