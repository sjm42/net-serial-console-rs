# Serial console TCP server/multiplexer and web client

## TCP Server

This small program is meant for sharing a serial port into network so that
it can be accessed with telnet, for example.

The console server allows basically unlimited number of concurrent client connections.
All clients will see the same serial port data, since it is replicated to all clients.

Write support has to be separately enabled with `-w` option.
Otherwise, all TCP connections are read only, i.e. nothing can be written into the serial port.
If write is enabled, any client can write to the console. The included web client does not
support writing. Something like `telnet` or `nc` have to be used for that.

A note about security: there is none. No ACL, no encryption, no authentication. Nothing.
You should probably use this only over SSH connections with TCP port forwarding
and limit the console server listening only on localhost.

This program was initially written just for learning Rust. It is kind of slightly improved
re-implementation of my old code written in Python.

```
USAGE:
    console-server [OPTIONS]

OPTIONS:
    -v, --verbose
    -d, --debug
    -t, --trace
    -h, --help       Prints help information
    -V, --version    Prints version information
    -w, --write      Enable client write to serial port

    -l, --listen <listen>               [default: 127.0.0.1:24242]
    -s, --ser-port <ser-port>           [default: /dev/ttyUSB0]
    -b, --baud <baud>                   [default: 115200]
        --ser-datab <ser-datab>         [default: 8]
        --ser-parity <ser-parity>       [default: N]
        --ser-stopb <ser-stopb>         [default: 1]
        --ser-flow <ser-flow>           [default: none]
```

## Web client

```
USAGE:
    console-web [OPTIONS]

OPTIONS:
    -v, --verbose
    -d, --debug
    -t, --trace
    -h, --help       Prints help information
    -V, --version    Prints version information

    -l, --listen <listen>       [default: 127.0.0.1:8080]
    -c, --connect <connect>     [default: 127.0.0.1:24242]
```

The web client starts a small HTTP server with Axum at the designated listen address.
The index page serves a simple HTML console window rendered from a Sailfish template
(`templates/console.html.stpl`).

The HTML console window utilizes a Server-Sent Events (SSE) stream provided by the
same web server at `/client` (or `/console/client`).

The event-stream output is line-oriented and uses a custom `Decoder` implementation
(`EventCodec`) derived from `tokio_util`'s `LinesCodec`, modified to wrap long lines
and replace non-printable characters with underscores.

Sample run:

```
$ ./target/debug/console-web
[2021-09-29T12:06:28Z INFO  net_serial_console::startup] Starting up Serial console web...
[2021-09-29T12:06:28Z INFO  console_web] Listening on 127.0.0.1:8080
[2021-09-29T12:07:31Z INFO  console_web] 127.0.0.1:54716 GET /console/client
```

Testing with `wget`:

```
$ wget -qS -O- http://localhost:8080/client
  HTTP/1.1 200 OK
  content-type: text/event-stream; charset=utf-8
  cache-control: no-cache
  transfer-encoding: chunked
  date: Wed, 29 Sep 2021 12:09:03 GMT
retry: 999999
id: 1
data: *** Connected to: /dev/ttyUSB0
```

Please note that there was also a console-server running at port 24242
to provide the actual serial port access.

## Program internals

### Overview

The project produces two binaries (`console-server` and `console-web`) that share a
common library crate. Both binaries manually construct a multi-threaded Tokio runtime
and shut it down with a 5-second timeout on exit.

### Source layout

```
src/
  lib.rs              -- Library root. Re-exports tracing macros and startup module.
  startup.rs          -- CLI option structs (Clap derive) and logging initialization.
  event.rs            -- EventCodec: custom tokio_util Decoder for SSE formatting.
  bin/
    console-server.rs -- TCP multiplexer for serial port access.
    console-web.rs    -- Axum HTTP server serving the web console frontend.
templates/
  console.html.stpl   -- Sailfish HTML template for the browser console page.
build.rs              -- Embeds git branch, commit, source timestamp, and rustc version.
```

### console-server

The server opens a serial port using `tokio-serial` with configurable baud rate, data
bits, parity, stop bits, and flow control. It then spawns two concurrent tasks:

1. **Serial I/O loop** (`handle_serial`) -- Runs a `tokio::select!` loop that:
   - Reads serial port data in 1 KB chunks and broadcasts it to all clients via a
     Tokio `broadcast` channel (capacity 256).
   - Receives data from clients via an `mpsc` channel and writes it to the serial port.

2. **TCP listener** (`handle_listener`) -- Accepts incoming TCP connections and spawns
   a task per client.

Each **client task** (`handle_client`) runs its own `tokio::select!` loop:
- Subscribes to the broadcast channel and forwards serial data to the client socket.
- Reads from the client socket. If `-w` (write) is enabled, forwards client input to
  the serial port via the shared `mpsc` channel. Otherwise, client input is discarded.

On connect, each client receives a greeting line: `*** Connected to: <serial-port>`.

The channel architecture:

```
                    broadcast (1-to-N)
  Serial Port  ──────────────────────────►  TCP Client 1
       ▲        ├────────────────────────►  TCP Client 2
       │        └────────────────────────►  TCP Client N
       │
       │              mpsc (N-to-1)
       └──────────────────────────────────  Any TCP Client (if -w enabled)
```

### console-web

At startup, the HTML template is rendered once into a string and stored in shared
application state (`AppCtx`) wrapped in `Arc`. The Axum router maps four routes:

| Route              | Handler   | Description                           |
|--------------------|-----------|---------------------------------------|
| `/`                | `index()` | Serves the pre-rendered HTML page     |
| `/console/`        | `index()` | Alias for the index page              |
| `/client`          | `client()`| SSE event stream                      |
| `/console/client`  | `client()`| Alias for the event stream            |

The `client()` handler opens a new TCP connection to `console-server` on each request.
It wraps the TCP stream in a `FramedRead` with `EventCodec` and returns it as a
streaming `Body`, which Axum delivers as a chunked `text/event-stream` response.

### EventCodec (`event.rs`)

A custom `tokio_util::codec::Decoder` that transforms raw serial byte streams into
SSE-formatted text events. Processing pipeline:

1. **Line detection** -- Scans the input buffer for `\n` bytes, buffering incomplete lines.
2. **Line wrapping** -- If a line exceeds 80 characters before a newline is found, it
   is split at the 80-character boundary and emitted immediately.
3. **Carriage return stripping** -- Trailing `\r` is removed from each line.
4. **Character sanitization** -- Non-printable ASCII characters (outside 0x20..0x7E) are
   replaced with underscores using `from_utf8_lossy()` followed by character replacement.
5. **Empty line suppression** -- Lines that are empty after stripping are silently dropped.
6. **SSE formatting** -- Each output line is wrapped in SSE format with an incrementing
   event ID and a large retry value (999999 ms) to effectively disable auto-reconnect:
   ```
   retry: 999999\r\n
   id: <n>\r\n
   data: <sanitized line>\r\n
   \r\n
   ```

### HTML template (`console.html.stpl`)

A minimal Sailfish template that renders a page with:
- A scrollable `<div>` (800px height) inside a `<pre>` block for monospace output.
- An `EventSource` JavaScript client that connects to the SSE endpoint, appends each
  received `data` field as a `<br>`-separated line, and auto-scrolls to the bottom.

### CLI and logging (`startup.rs`)

CLI argument parsing uses Clap's derive API. `OptsCommon` is a shared struct (flattened
into both binary-specific option structs) that provides `-v`/`-d`/`-t` flags mapping to
tracing log levels: ERROR (default), INFO, DEBUG, TRACE.

On startup, `start_pgm()` initializes `tracing-subscriber` and logs build metadata
(git branch, commit hash, source timestamp, compiler version) embedded at compile time
by `build.rs` via the `build-data` crate.
