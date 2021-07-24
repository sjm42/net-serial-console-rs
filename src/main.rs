// main.rs

use log::*;
use serial::SerialPort;
use std::io::{ErrorKind, Read, Write};
use std::{error::Error, net::SocketAddr, sync::Arc, time};
use structopt::StructOpt;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::{net, task};

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

fn opt_baudrate(baudrate: u32) -> Result<serial::BaudRate, serial::Error> {
    match baudrate {
        110 => Ok(serial::Baud110),
        300 => Ok(serial::Baud300),
        600 => Ok(serial::Baud600),
        1200 => Ok(serial::Baud1200),
        2400 => Ok(serial::Baud2400),
        4800 => Ok(serial::Baud4800),
        9600 => Ok(serial::Baud9600),
        19200 => Ok(serial::Baud19200),
        38400 => Ok(serial::Baud38400),
        57600 => Ok(serial::Baud57600),
        115200 => Ok(serial::Baud115200),
        _ => Err(serial::Error::new(
            serial::ErrorKind::InvalidInput,
            "invalid baudrate",
        )),
    }
}

fn opt_flowcontrol(flowcontrol: &str) -> Result<serial::FlowControl, serial::Error> {
    match flowcontrol {
        "N" | "n" | "NONE" | "none" => Ok(serial::FlowNone),
        "H" | "h" | "HARD" | "hard" | "hw" | "hardware" => Ok(serial::FlowHardware),
        "S" | "s" | "SOFT" | "soft" | "sw" | "software" => Ok(serial::FlowControl::FlowSoftware),
        _ => Err(serial::Error::new(
            serial::ErrorKind::InvalidInput,
            "invalid flowcontrol",
        )),
    }
}

fn opt_databits(bits: u32) -> Result<serial::CharSize, serial::Error> {
    match bits {
        5 => Ok(serial::Bits5),
        6 => Ok(serial::Bits6),
        7 => Ok(serial::Bits7),
        8 => Ok(serial::Bits8),
        _ => Err(serial::Error::new(
            serial::ErrorKind::InvalidInput,
            "invalid databits",
        )),
    }
}

fn opt_parity(parity: &str) -> Result<serial::Parity, serial::Error> {
    match parity {
        "N" | "n" => Ok(serial::ParityNone),
        "E" | "e" => Ok(serial::ParityEven),
        "O" | "o" => Ok(serial::ParityOdd),
        _ => Err(serial::Error::new(
            serial::ErrorKind::InvalidInput,
            "invalid parity",
        )),
    }
}

fn opt_stopbits(bits: u32) -> Result<serial::StopBits, serial::Error> {
    // let foo = serial::Error::new("");
    match bits {
        1 => Ok(serial::Stop1),
        2 => Ok(serial::Stop2),
        _ => Err(serial::Error::new(
            serial::ErrorKind::InvalidInput,
            "invalid stopbits",
        )),
    }
}

async fn run_server(port: serial::SystemPort, opt: GlobalServerOptions) -> tokio::io::Result<()> {
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
    // create the serial handler itself
    tokio::spawn(async move { handle_serial(port, ser_read_atx, write_rx).await.unwrap() });

    let listener = net::TcpListener::bind(&opt.listen).await?;
    info!("Listening on {}", &opt.listen);
    loop {
        let (sock, addr) = listener.accept().await?;
        let ser_name = opt.serial_port.clone();
        let client_read_atx = Arc::clone(&read_atx);
        let client_write_atx = Arc::clone(&write_atx);
        let write_enabled = opt.write;
        tokio::spawn(async move {
            handle_client(
                ser_name,
                sock,
                addr,
                client_read_atx,
                client_write_atx,
                write_enabled,
            )
            .await
            .unwrap()
        });
        task::yield_now().await;
    }
}

async fn handle_serial(
    mut port: serial::SystemPort,
    a_send: Arc<RwLock<broadcast::Sender<String>>>,
    mut a_recv: mpsc::Receiver<String>,
) -> tokio::io::Result<()> {
    info!("Starting serial IO");
    let mut buf = [0; BUFSZ];
    loop {
        let res = port.read(&mut buf);
        match res {
            Ok(n) => {
                let s = String::from_utf8_lossy(&buf[0..n]).to_string();
                debug!("Serial read {} bytes.", n);
                // eprint!("{}", &s);
                {
                    let tx = a_send.write().await;
                    tx.send(s).unwrap();
                }
            }
            Err(e) if e.kind() == io::ErrorKind::TimedOut => {}
            Err(e) => {
                error!("Error {:?}", e);
                return Err(e);
            }
        }
        tokio::select! {
            Some(msg) = a_recv.recv() => {
                debug!("serial write: <-- {} bytes", msg.len());
                port.write_all(msg.as_bytes())?;
            }
        }
        task::yield_now().await;
    }
}

async fn handle_client(
    ser_name: String,
    mut sock: net::TcpStream,
    addr: SocketAddr,
    a_bsender: Arc<RwLock<broadcast::Sender<String>>>,
    tx: Arc<RwLock<mpsc::Sender<String>>>,
    write_enabled: bool,
) -> tokio::io::Result<()> {
    info!("Client handler: connection from {}", addr);
    let mut buf = [0; BUFSZ];
    sock.write(format!("\n*** Connected to: {}\n\n", &ser_name).as_bytes())
        .await?;
    let mut rx;
    {
        // create a channel receiver for us
        rx = a_bsender.write().await.subscribe();
    }

    loop {
        tokio::select! {
            res = rx.recv() => {
                match res {
                    Ok(msg) => {
                        sock.write_all(msg.as_bytes()).await?;
                        sock.flush().await?;
                    }
                    Err(_e) => {
                        return Err(io::Error::new(ErrorKind::NotConnected,
                            "serial port gone?"));
                    }
                }
            }
            res = sock.read(&mut buf) => {
                match res {
                    Ok(len) => {
                        debug!("Socket read: {} -> {} bytes", addr, len);
                        // We only react to client input if write_enabled flag is set
                        // otherwise, data from socket is just thrown away
                        if write_enabled {
                            let s = String::from_utf8_lossy(&buf[0..len]).to_string();
                            let tx = tx.write().await;
                            match tx.send(s).await {
                                Ok(_) => { }
                                Err(_e) => {
                                    return Err(io::Error::new(ErrorKind::NotConnected,
                                        "serial port gone?"));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }

            }
        }
        task::yield_now().await;
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

    let mut ser_port = serial::open(&opt.serial_port)?;
    ser_port.reconfigure(&|s| {
        s.set_flow_control(opt_flowcontrol(&opt.ser_flow)?);
        s.set_baud_rate(opt_baudrate(opt.ser_baud)?)?;
        s.set_char_size(opt_databits(opt.ser_datab)?);
        s.set_parity(opt_parity(&opt.ser_parity)?);
        s.set_stop_bits(opt_stopbits(opt.ser_stopb)?);
        Ok(())
    })?;
    SerialPort::set_timeout(&mut ser_port, time::Duration::new(0, 20000000))?;

    info!(
        "Opened serial port {} with write {}abled.",
        &opt.serial_port,
        if opt.write { "en" } else { "dis" }
    );
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async move {
        run_server(ser_port, opt).await.unwrap();
    });
    rt.shutdown_timeout(time::Duration::new(5, 0));
    Ok(())
}
// EOF
