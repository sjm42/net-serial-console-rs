// startup.rs

use log::*;
use std::{env, error::Error};
use structopt::StructOpt;

#[derive(Clone, Debug, Default, StructOpt)]
pub struct OptsCommon {
    #[structopt(short, long)]
    pub debug: bool,
    #[structopt(short, long)]
    pub trace: bool,
}
impl OptsCommon {
    pub fn finish(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
    fn get_loglevel(&self) -> LevelFilter {
        if self.trace {
            LevelFilter::Trace
        } else if self.debug {
            LevelFilter::Debug
        } else {
            LevelFilter::Info
        }
    }
}

#[derive(Clone, Debug, Default, StructOpt)]
pub struct OptsConsoleServer {
    #[structopt(flatten)]
    pub c: OptsCommon,
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
impl OptsConsoleServer {
    pub fn finish(&mut self) -> Result<(), Box<dyn Error>> {
        self.c.finish()?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default, StructOpt)]
pub struct OptsConsoleWeb {
    #[structopt(flatten)]
    pub c: OptsCommon,
    #[structopt(short, long, default_value = "127.0.0.1:8080")]
    pub listen: String,
    #[structopt(short, long, default_value = "127.0.0.1:24242")]
    pub connect: String,
}
impl OptsConsoleWeb {
    pub fn finish(&mut self) -> Result<(), Box<dyn Error>> {
        self.c.finish()?;
        Ok(())
    }
}

pub fn expand_home(pathname: &mut String) -> Result<(), Box<dyn Error>> {
    let home = env::var("HOME")?;
    *pathname = pathname.as_str().replace("$HOME", &home);
    Ok(())
}

pub fn start_pgm(c: &OptsCommon, desc: &str) {
    env_logger::Builder::new()
        .filter_level(c.get_loglevel())
        .format_timestamp_secs()
        .init();
    info!("Starting up {}...", desc);
    debug!("Git branch: {}", env!("GIT_BRANCH"));
    debug!("Git commit: {}", env!("GIT_COMMIT"));
    debug!("Source timestamp: {}", env!("SOURCE_TIMESTAMP"));
    debug!("Compiler version: {}", env!("RUSTC_VERSION"));
}

// EOF
