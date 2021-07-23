// main.rs

use log::*;
use serial::SerialPort;
use std::{error::Error, io::Read, net::SocketAddr, sync::Arc, time};
use structopt::StructOpt;
use tokio::io::{self, AsyncWriteExt};
use tokio::{net, task};
use tokio::sync::{broadcast, RwLock};

const BUFSZ: usize = 1024;
const CHANSZ: usize = 16;

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

async fn run_server(port: serial::SystemPort, addr: String) -> tokio::io::Result<()> {
    let (tx, _rx) = broadcast::channel(CHANSZ);
    let atx = Arc::new(RwLock::new(tx));
    let ser_atx = Arc::clone(&atx);
    tokio::spawn(async move { handle_serial(port, ser_atx).await.unwrap() });

    let listener = net::TcpListener::bind(&addr).await?;
    info!("Listening on {}", &addr);
    loop {
        let (sock, addr) = listener.accept().await?;
        let client_atx = Arc::clone(&atx);
        tokio::spawn(async move { handle_client(sock, addr, client_atx).await.unwrap() });
        task::yield_now().await;
    }
}

async fn handle_serial(
    mut port: serial::SystemPort,
    atx: Arc<RwLock<broadcast::Sender<String>>>,
) -> tokio::io::Result<()> {
    let mut buf = [0; BUFSZ];
    info!("Starting serial read...");
    loop {
        let res = port.read(&mut buf);
        match res {
            Ok(n) => {
                let s = String::from_utf8_lossy(&buf[0..n]);
                // info!("Read {} bytes.", n);
                // info!("serial: {}", String::from_utf8_lossy(&buf[0..n]));
                eprint!("{}", &s);
                {
                    let tx = atx.write().await;
                    tx.send(s.to_string()).unwrap();
                }
            }
            Err(e) if e.kind() == io::ErrorKind::TimedOut => { }
            Err(e) => {
                info!("Error {:?}", e);
                return Err(e);
            }
        }
        task::yield_now().await;
    }
}

async fn handle_client(
    mut sock: net::TcpStream,
    addr: SocketAddr,
    atx: Arc<RwLock<broadcast::Sender<String>>>,
) -> tokio::io::Result<()> {
    info!("Client handler: connection from {}", addr);
    sock.write("\n*** Hello!\n\n".as_bytes()).await?;
    let mut rx;
    {
        rx = atx.write().await.subscribe();
    }

    loop {
        let msg = rx.recv().await.unwrap();
        sock.write(&msg.as_bytes()).await?;
        sock.flush().await?;
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

    info!("Opened serial port.");
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async move {
        run_server(ser_port, opt.listen.clone()).await.unwrap();
    });
    rt.shutdown_timeout(time::Duration::new(5, 0));
    Ok(())
}
// EOF
