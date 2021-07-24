// main.rs

use log::*;
use std::{error::Error, net::SocketAddr, process, sync::Arc, time};
use structopt::StructOpt;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio_serial::{self, SerialPortBuilderExt};

const BUFSZ: usize = 1024;
const CHANSZ: usize = 256;

#[derive(Debug, StructOpt)]
pub struct GlobalServerOptions {
    #[structopt(short, long)]
    pub debug: bool,
    #[structopt(short, long)]
    pub trace: bool,
    #[structopt(short, long, default_value = "127.0.0.1:24242")]
    pub listen: String,
    #[structopt(short, long, default_value = "/dev/ttyUSB0")]
    pub serial_port: String,
    #[structopt(long, default_value = "none")]
    pub ser_flow: String,
    #[structopt(short = "b", long, default_value = "9600")]
    pub ser_baud: u32,
    #[structopt(long, default_value = "8")]
    pub ser_datab: u32,
    #[structopt(long, default_value = "N")]
    pub ser_parity: String,
    #[structopt(long, default_value = "1")]
    pub ser_stopb: u32,
    #[structopt(short, long)]
    pub write: bool,
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
    // let foo = serial::Error::new("");
    match bits {
        1 => Ok(tokio_serial::StopBits::One),
        2 => Ok(tokio_serial::StopBits::Two),
        _ => Err(tokio_serial::Error::new(
            tokio_serial::ErrorKind::InvalidInput,
            "invalid stopbits",
        )),
    }
}

async fn run_server(opt: GlobalServerOptions) -> io::Result<()> {
    let port = tokio_serial::new(&opt.serial_port, opt.ser_baud)
        .flow_control(opt_flowcontrol(&opt.ser_flow)?)
        .data_bits(opt_databits(opt.ser_datab)?)
        .parity(opt_parity(&opt.ser_parity)?)
        .stop_bits(opt_stopbits(opt.ser_stopb)?)
        .open_native_async()?;
    info!(
        "Opened serial port {} with write {}abled.",
        &opt.serial_port,
        if opt.write { "en" } else { "dis" }
    );

    // Note: here read/write in variable naming is referring to the serial port data direction

    // create a broadcast channel for sending serial msgs to all clients
    let (read_tx, _read_rx) = broadcast::channel(CHANSZ);
    // ...and put the channel sender inside Arc+RwLock to be able to move it
    let read_atx = Arc::new(RwLock::new(read_tx));

    // create an mpsc channel for receiving serial port input from any client
    // mpsc = multi-producer, single consumer queue
    let (write_tx, write_rx) = mpsc::channel(CHANSZ);
    // ...and put the channel sender inside Arc+RwLock to be able to move it
    let write_atx = Arc::new(RwLock::new(write_tx));
    // create channel clone for serial handler
    let ser_read_atx = Arc::clone(&read_atx);

    tokio::spawn(async move {
        handle_listener(opt, read_atx, write_atx).await.unwrap();
    });
    handle_serial(port, ser_read_atx, write_rx).await
}

async fn handle_listener(
    opt: GlobalServerOptions,
    read_atx: Arc<RwLock<broadcast::Sender<String>>>,
    write_atx: Arc<RwLock<mpsc::Sender<String>>>,
) -> io::Result<()> {
    let listener;
    match net::TcpListener::bind(&opt.listen).await {
        Ok(l) => {
            listener = l;
        }
        Err(e) => {
            error!("Failed to listen {}", &opt.listen);
            error!("{}", e);
            error!("Exit.");
            process::exit(1);
        }
    }
    info!("Listening on {}", &opt.listen);
    loop {
        let res = listener.accept().await;
        match res {
            Ok((sock, addr)) => {
                let ser_name = opt.serial_port.clone();
                let write_enabled = opt.write;
                let client_read_atx = Arc::clone(&read_atx);
                let client_write_atx = Arc::clone(&write_atx);
                tokio::spawn(async move {
                    handle_client(
                        ser_name,
                        write_enabled,
                        sock,
                        addr,
                        client_read_atx,
                        client_write_atx,
                    )
                    .await
                    .unwrap()
                });
            }
            Err(e) => {
                error!("Accept failed: {}", e);
            }
        }
    }
}

async fn handle_serial(
    mut port: tokio_serial::SerialStream,
    a_send: Arc<RwLock<broadcast::Sender<String>>>,
    mut a_recv: mpsc::Receiver<String>,
) -> io::Result<()> {
    info!("Starting serial IO");

    let mut buf = [0; BUFSZ];
    loop {
        tokio::select! {
            res = port.read(&mut buf) => {
                match res {
                    Ok(n) => {
                        if n == 0 {
                            info!("Serial disconnected.");
                            return Ok(());
                        }
                        debug!("Serial read {} bytes.", n);
                        let s = String::from_utf8_lossy(&buf[0..n]).to_string();
                        let tx = a_send.write().await;
                        tx.send(s).unwrap();
                        }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
            Some(msg) = a_recv.recv() => {
                debug!("serial write {} bytes", msg.len());
                port.write_all(msg.as_bytes()).await?;
            }
        }
    }
}

async fn handle_client(
    ser_name: String,
    write_enabled: bool,
    mut sock: net::TcpStream,
    addr: SocketAddr,
    a_bsender: Arc<RwLock<broadcast::Sender<String>>>,
    tx: Arc<RwLock<mpsc::Sender<String>>>,
) -> io::Result<()> {
    info!("Client connection from {}", addr);

    let mut rx;
    {
        // create a channel receiver for us
        rx = a_bsender.write().await.subscribe();
    }
    let mut buf = [0; BUFSZ];
    sock.write_all(format!("*** Connected to: {}\n", &ser_name).as_bytes())
        .await?;

    loop {
        tokio::select! {
            Ok(msg) = rx.recv() => {
                sock.write_all(msg.as_bytes()).await?;
                sock.flush().await?;
            }
            res = sock.read(&mut buf) => {
                match res {
                    Ok(n) => {
                        if n == 0 {
                            info!("Client disconnected: {}", addr);
                            return Ok(());
                        }
                        debug!("Socket read: {} bytes <-- {}", n, addr);
                        // We only react to client input if write_enabled flag is set
                        // otherwise, data from socket is just thrown away
                        if write_enabled {
                            let s = String::from_utf8_lossy(&buf[0..n]).to_string();
                            {
                                let tx = tx.write().await;
                                tx.send(s).await.unwrap();
                            }
                        }
                    }
                    Err(e) => { return Err(e); }
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let opt: GlobalServerOptions = GlobalServerOptions::from_args();
    let loglevel = if opt.trace {
        LevelFilter::Trace
    } else if opt.debug {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    env_logger::Builder::new()
        .filter_level(loglevel)
        .format_timestamp_secs()
        .init();
    info!("Starting up console-server...");
    info!("Git branch: {}", env!("GIT_BRANCH"));
    info!("Git commit: {}", env!("GIT_COMMIT"));
    info!("Source timestamp: {}", env!("SOURCE_TIMESTAMP"));
    info!("Compiler version: {}", env!("RUSTC_VERSION"));

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async move {
        run_server(opt).await.unwrap();
    });
    rt.shutdown_timeout(time::Duration::new(5, 0));
    info!("Exit.");
    Ok(())
}
// EOF
