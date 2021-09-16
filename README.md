# A serial console tcp server and web client in Rust

## TCP Server

This small program is meant for sharing a serial port into network so that
it can be accessed with telnet, for example.

The console server allows basically unlimited number of concurrent client connections.
Write support has to be separately enabled with -w option.
Otherwise, all tcp connections are read only, i.e. nothing can be written into the serial port.

A note about security: there is none. No ACL, no encryption, no authentication. Nothing.
You should probably use this only over ssh connections with tcp port forwarding
and limit the console server listening only on localhost.

This program was initially written just for learning Rust. It is kind of slightly improved
re-implementation of old code written in Python.

```
USAGE:
    console-server [FLAGS] [OPTIONS]

FLAGS:
    -d, --debug
    -h, --help       Prints help information
    -t, --trace
    -V, --version    Prints version information
    -w, --write

OPTIONS:
    -l, --listen <listen>               [default: 127.0.0.1:24242]
    -b, --ser-baud <ser-baud>           [default: 9600]
        --ser-datab <ser-datab>         [default: 8]
        --ser-flow <ser-flow>           [default: none]
        --ser-parity <ser-parity>       [default: N]
        --ser-stopb <ser-stopb>         [default: 1]
    -s, --serial-port <serial-port>     [default: /dev/ttyUSB0]

```

## Web client

```
USAGE:
    console-client [FLAGS] [OPTIONS]

FLAGS:
    -d, --debug
    -h, --help       Prints help information
    -t, --trace
    -V, --version    Prints version information

OPTIONS:
    -c, --connect <connect>               [default: 127.0.0.1:24242]
    -l, --listen <listen>                 [default: 127.0.0.1:8080]
        --template-dir <template-dir>     [default: templates]
```

The console client starts a small internal web server with `hyper` at the designated listen address.
The index page from `index()` is simple, and includes HTML code
to create a line-oriented console window. The HTML is rendered from a Sailfish template.

HTML console window is utilizing an event-stream that is provided by the same web-server
from `client()`.

Rudimentary URL/request routing is handled with the function `req_router()`.

The event-stream output is line-oriented and using a DIY input Decoder that was basically stolen
from `tokio_util` `LinesCodec` and modified heavily to wrap long lines and replace non-printable
characters with underscores. It is a bit brutal but works.

Sample run:

```
$ ./target/debug/console-client
[2021-07-30T10:09:41Z INFO  console_client] Starting up console-client...
[2021-07-30T10:09:41Z INFO  console_client] Template directory: templates
[2021-07-30T10:09:42Z INFO  console_client] Found templates: [console.html.tera]
[2021-07-30T10:09:44Z INFO  console_client] 127.0.0.1:40968 GET /client/
```

testing with `wget`:

```
$ wget -qS -O- http://localhost:8080/client/
  HTTP/1.1 200 OK
  content-type: text/event-stream; charset=utf-8
  cache-control: no-cache
  transfer-encoding: chunked
  date: Fri, 30 Jul 2021 10:09:44 GMT
retry: 999999
id: 1
data: *** Connected to: /dev/ttyUSB0

```

Please note that there was also a console-server running at port 24242
to provide the actual serial port access.
