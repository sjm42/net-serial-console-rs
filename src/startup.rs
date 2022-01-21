// startup.rs

use log::*;
use std::env;
use structopt::StructOpt;

#[derive(Clone, Debug, Default, StructOpt)]
pub struct OptsCommon {
    #[structopt(short, long)]
    pub debug: bool,
    #[structopt(short, long)]
    pub trace: bool,
}
impl OptsCommon {
    pub fn finish(&mut self) -> anyhow::Result<()> {
        // do sanity checking or env var expansion etc...
        Ok(())
    }
    pub fn get_loglevel(&self) -> LevelFilter {
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
    pub ser_port: String,
    #[structopt(long, default_value = "none")]
    pub ser_flow: String,
    #[structopt(short = "b", long, default_value = "115200")]
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
    pub fn finish(&mut self) -> anyhow::Result<()> {
        self.c.finish()
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
    pub fn finish(&mut self) -> anyhow::Result<()> {
        self.c.finish()
    }
}

pub fn start_pgm(opts: &OptsCommon, desc: &str) {
    env_logger::Builder::new()
        .filter_level(opts.get_loglevel())
        .format_timestamp_secs()
        .init();
    info!("Starting up {desc}...");
    debug!("Git branch: {}", env!("GIT_BRANCH"));
    debug!("Git commit: {}", env!("GIT_COMMIT"));
    debug!("Source timestamp: {}", env!("SOURCE_TIMESTAMP"));
    debug!("Compiler version: {}", env!("RUSTC_VERSION"));
}
// EOF
