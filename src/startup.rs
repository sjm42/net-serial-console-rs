// startup.rs

use std::env;

use clap::{Args, Parser};

use crate::*;

#[derive(Clone, Debug, Default, Args)]
pub struct OptsCommon {
    #[arg(short, long)]
    pub verbose: bool,

    #[arg(short, long)]
    pub debug: bool,

    #[arg(short, long)]
    pub trace: bool,
}

impl OptsCommon {
    pub fn finalize(&mut self) -> anyhow::Result<()> {
        // do sanity checking or env var expansion etc...
        Ok(())
    }

    pub fn get_loglevel(&self) -> Level {
        if self.trace {
            Level::TRACE
        } else if self.debug {
            Level::DEBUG
        } else if self.verbose {
            Level::INFO
        } else {
            Level::ERROR
        }
    }

    pub fn start_pgm(&self, name: &str) {
        tracing_subscriber::fmt()
            .with_max_level(self.get_loglevel())
            .with_target(false)
            .init();

        info!("Starting up {name}...");
        debug!("Git branch: {}", env!("GIT_BRANCH"));
        debug!("Git commit: {}", env!("GIT_COMMIT"));
        debug!("Source timestamp: {}", env!("SOURCE_TIMESTAMP"));
        debug!("Compiler version: {}", env!("RUSTC_VERSION"));
    }
}

#[derive(Clone, Debug, Default, Parser)]
pub struct OptsConsoleServer {
    #[command(flatten)]
    pub c: OptsCommon,

    #[arg(short, long, default_value = "127.0.0.1:24242")]
    pub listen: String,
    #[arg(short, long, default_value = "/dev/ttyUSB0")]
    pub ser_port: String,
    #[arg(long, default_value = "none")]
    pub ser_flow: String,
    #[arg(short, long, default_value_t = 115200)]
    pub baud: u32,
    #[arg(long, default_value_t = 8)]
    pub ser_datab: u32,
    #[arg(long, default_value = "N")]
    pub ser_parity: String,
    #[arg(long, default_value_t = 1)]
    pub ser_stopb: u32,
    #[arg(short, long)]
    pub write: bool,
}

impl OptsConsoleServer {
    pub fn finalize(&mut self) -> anyhow::Result<()> {
        self.c.finalize()
    }
}

#[derive(Clone, Debug, Default, Parser)]
pub struct OptsConsoleWeb {
    #[command(flatten)]
    pub c: OptsCommon,

    #[arg(short, long, default_value = "127.0.0.1:8080")]
    pub listen: String,
    #[arg(short, long, default_value = "127.0.0.1:24242")]
    pub connect: String,
}

impl OptsConsoleWeb {
    pub fn finalize(&mut self) -> anyhow::Result<()> {
        self.c.finalize()
    }
}


// EOF
