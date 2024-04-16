// console-server.rs

use std::{net::SocketAddr, time};

use anyhow::anyhow;
use clap::Parser;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net;
use tokio::sync::{broadcast, mpsc};
use tokio_serial::{self, SerialPortBuilderExt};

use net_serial_console::*;

const BUFSZ: usize = 1024;
const CHANSZ: usize = 256;

fn main() -> anyhow::Result<()> {
    let mut opts = OptsConsoleServer::parse();
    opts.finalize()?;
    opts.c.start_pgm(env!("CARGO_BIN_NAME"));

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        if let Err(e) = run_server(opts).await {
            error!("Error: {}", e);
        }
    });
    runtime.shutdown_timeout(time::Duration::new(5, 0));
    info!("Exit.");
    Ok(())
}

async fn run_server(opts: OptsConsoleServer) -> anyhow::Result<()> {
    let port = tokio_serial::new(&opts.ser_port, opts.baud)
        .flow_control(opt_flowcontrol(&opts.ser_flow)?)
        .data_bits(opt_databits(opts.ser_datab)?)
        .parity(opt_parity(&opts.ser_parity)?)
        .stop_bits(opt_stopbits(opts.ser_stopb)?)
        .open_native_async()?;
    info!(
        "Opened serial port {} with write {}abled.",
        &opts.ser_port,
        if opts.write { "en" } else { "dis" }
    );

    // Note: here read/write in variable naming is referring to the serial port data direction

    // create a broadcast channel for sending serial msgs to all clients
    let (read_tx, _read_rx) = broadcast::channel(CHANSZ);

    // create an mpsc channel for receiving serial port input from any client
    // mpsc = multi-producer, single consumer queue
    let (write_tx, write_rx) = mpsc::channel(CHANSZ);

    tokio::spawn(handle_listener(opts, read_tx.clone(), write_tx));
    handle_serial(port, read_tx, write_rx).await
}

async fn handle_serial(
    mut port: tokio_serial::SerialStream,
    a_send: broadcast::Sender<Vec<u8>>,
    mut a_recv: mpsc::Receiver<Vec<u8>>,
) -> anyhow::Result<()> {
    info!("Starting serial IO");

    let mut buf = [0; BUFSZ];
    loop {
        tokio::select! {
            Some(msg) = a_recv.recv() => {
                debug!("serial write {} bytes", msg.len());
                port.write_all(msg.as_ref()).await?;
            }

            res = port.read(&mut buf) => {
                match res {
                    Ok(0) => {
                        info!("Serial <EOF>");
                        return Ok(());
                    }
                    Ok(n) => {
                        debug!("Serial read {n} bytes.");
                        a_send.send(buf[0..n].to_owned())?;
                    }
                    Err(e) => {
                        return Err(anyhow!(e));
                    }
                }
            }
        }
    }
}

async fn handle_listener(
    opts: OptsConsoleServer,
    read_atx: broadcast::Sender<Vec<u8>>,
    write_atx: mpsc::Sender<Vec<u8>>,
) -> anyhow::Result<()> {
    let listener = net::TcpListener::bind(&opts.listen).await?;
    info!("Listening on {}", &opts.listen);

    loop {
        let (sock, addr) = match listener.accept().await {
            Err(e) => {
                error!("accept failed: {e:?}");
                continue;
            }
            Ok(x) => x,
        };

        let ser_name = opts.ser_port.clone();
        let write_enabled = opts.write;
        let client_read_atx = read_atx.subscribe();
        let client_write_atx = write_atx.clone();
        tokio::spawn(async move {
            let ret = handle_client(
                ser_name,
                write_enabled,
                sock,
                addr,
                client_read_atx,
                client_write_atx,
            )
                .await;
            if let Err(e) = ret {
                // log errors
                error!("client error: {e:?}");
            }
        });
    }
}

async fn handle_client(
    ser_name: String,
    write_enabled: bool,
    mut sock: net::TcpStream,
    addr: SocketAddr,
    mut rx: broadcast::Receiver<Vec<u8>>,
    tx: mpsc::Sender<Vec<u8>>,
) -> anyhow::Result<()> {
    info!("Client connection from {addr:?}");

    let mut buf = [0; BUFSZ];
    sock.write_all(format!("*** Connected to: {ser_name}\n").as_bytes())
        .await?;

    loop {
        tokio::select! {
            Ok(msg) = rx.recv() => {
                sock.write_all(msg.as_ref()).await?;
                sock.flush().await?;
            }

            res = sock.read(&mut buf) => {
                let n = match res {
                    Err(e) => {
                        return Err(anyhow!(e));
                    },
                    Ok(x) => x
                };

                if n == 0 {
                    info!("Client disconnected: {addr:?}");
                    return Ok(());
                }
                debug!("Socket read: {n} bytes <-- {addr:?}");
                // We only react to client input if write_enabled flag is set
                // otherwise, data from socket is just thrown away
                if write_enabled {
                    tx.send(buf[0..n].to_owned()).await?;
                }
            }
        }
    }
}

fn opt_flowcontrol(flowcontrol: &str) -> tokio_serial::Result<tokio_serial::FlowControl> {
    match flowcontrol {
        "N" | "n" | "NONE" | "none" => Ok(tokio_serial::FlowControl::None),
        "H" | "h" | "HARD" | "hard" | "hw" | "hardware" => Ok(tokio_serial::FlowControl::Hardware),
        "S" | "s" | "SOFT" | "soft" | "sw" | "software" => Ok(tokio_serial::FlowControl::Software),
        _ => Err(tokio_serial::Error::new(
            tokio_serial::ErrorKind::InvalidInput,
            "invalid flowcontrol",
        )),
    }
}

fn opt_databits(bits: u32) -> tokio_serial::Result<tokio_serial::DataBits> {
    match bits {
        5 => Ok(tokio_serial::DataBits::Five),
        6 => Ok(tokio_serial::DataBits::Six),
        7 => Ok(tokio_serial::DataBits::Seven),
        8 => Ok(tokio_serial::DataBits::Eight),
        _ => Err(tokio_serial::Error::new(
            tokio_serial::ErrorKind::InvalidInput,
            "invalid databits",
        )),
    }
}

fn opt_parity(parity: &str) -> tokio_serial::Result<tokio_serial::Parity> {
    match parity {
        "N" | "n" => Ok(tokio_serial::Parity::None),
        "E" | "e" => Ok(tokio_serial::Parity::Even),
        "O" | "o" => Ok(tokio_serial::Parity::Odd),
        _ => Err(tokio_serial::Error::new(
            tokio_serial::ErrorKind::InvalidInput,
            "invalid parity",
        )),
    }
}

fn opt_stopbits(bits: u32) -> tokio_serial::Result<tokio_serial::StopBits> {
    match bits {
        1 => Ok(tokio_serial::StopBits::One),
        2 => Ok(tokio_serial::StopBits::Two),
        _ => Err(tokio_serial::Error::new(
            tokio_serial::ErrorKind::InvalidInput,
            "invalid stopbits",
        )),
    }
}
// EOF
