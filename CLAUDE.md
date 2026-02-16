# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Serial console TCP server/multiplexer and web client written in Rust. Shares a single serial port over the network for remote access (like telnet). Supports unlimited concurrent read-only clients with optional write support (`-w` flag). Includes a web frontend using Server-Sent Events (SSE). No authentication/encryption — intended for use over SSH tunnels.

## Build Commands

```bash
cargo build                        # Debug build
cargo build --release              # Release build (fat LTO, opt-level 3)
cargo run --bin console-server     # Run TCP server
cargo run --bin console-web        # Run web frontend
```

No tests, linter config, or CI exist in this project.

## Architecture

**Two binaries** sharing a common library:

- **console-server** (`src/bin/console-server.rs`) — Opens a serial port and listens for TCP connections. Broadcasts serial data to all clients via a Tokio broadcast channel (capacity 256). Write path uses an MPSC channel. Reads serial data in 1KB chunks.

- **console-web** (`src/bin/console-web.rs`) — Axum HTTP server that connects to console-server as a TCP client. Serves an HTML page with an SSE stream (`/client` or `/console/client`). The HTML template is in `templates/console.html.stpl` (Sailfish).

**Library modules** (`src/`):
- `startup.rs` — Clap-derived CLI option structs (`OptsCommon`, `OptsConsoleServer`, `OptsConsoleWeb`) and tracing-subscriber logging init.
- `event.rs` — `EventCodec` implementing `tokio_util::codec::Decoder`. Wraps lines at 80 chars, strips `\r`, replaces non-printable ASCII with underscores, and formats output as SSE with incrementing event IDs.

**Build script** (`build.rs`) — Uses `build-data` crate to embed git branch, commit, source timestamp, and rustc version at compile time.

## Key Defaults

- Serial port: `/dev/ttyUSB0`, 115200 baud, 8N1, no flow control
- TCP server: `127.0.0.1:24242`
- Web server: `127.0.0.1:8080`

## Toolchain

Stable Rust (specified in `rust-toolchain.toml`), edition 2024.
