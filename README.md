# Small serial console server in Rust

This small program is meant for sharing a serial port into network so that
it can be accessed with telnet, for example.

The console server allows basically unlimited number of concurrent client connections.
Write support has to be separately enabled with -w option.
Otherwise, all tcp connections are read only, i.e. nothing can be written into the serial port.

A note about security: there is none. No ACL, no encryption, no authentication. Nothing.
You should probably use this only over ssh connections with tcp port forwarding
and limit the console server listening only on localhost.

This programm was initially made just for learning Rust. It is a kind of slightly improved
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
