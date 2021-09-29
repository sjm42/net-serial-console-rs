# Serial console tcp server+multiplexer and web client

## TCP Server

This small program is meant for sharing a serial port into network so that
it can be accessed with telnet, for example.

The console server allows basically unlimited number of concurrent client connections.
All clients will see the same serial port data, since it is replicated to all clients.

Write support has to be separately enabled with -w option.
Otherwise, all tcp connections are read only, i.e. nothing can be written into the serial port.
If write is enabled, any client can write to the console. The included web client does not
support writing. Something like `telnet` or `nc` have to be used for that.

A note about security: there is none. No ACL, no encryption, no authentication. Nothing.
You should probably use this only over ssh connections with tcp port forwarding
and limit the console server listening only on localhost.

This program was initially written just for learning Rust. It is kind of slightly improved
re-implementation of my old code written in Python.

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
    console-web [FLAGS] [OPTIONS]

FLAGS:
    -d, --debug
    -h, --help       Prints help information
    -t, --trace
    -V, --version    Prints version information

OPTIONS:
    -c, --connect <connect>     [default: 127.0.0.1:24242]
    -l, --listen <listen>       [default: 127.0.0.1:8080]
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
$ ./target/debug/console-web
[2021-09-29T12:06:28Z INFO  net_serial_console::startup] Starting up Serial console web...
[2021-09-29T12:06:28Z INFO  console_web] Listening on 127.0.0.1:8080
[2021-09-29T12:07:31Z INFO  console_web] 127.0.0.1:54716 GET /console/client
```

testing with `wget`:

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
