// main.rs

// use sailfish::TemplateOnce;

use bytes::BytesMut;
use hyper::service::{make_service_fn, service_fn};
use hyper::{server::conn::AddrStream, Body, Method, Request, Response, Server, StatusCode};
use log::*;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use sailfish::TemplateOnce;
use std::{convert::Infallible, default::Default, error::Error, net::SocketAddr, time};
use structopt::StructOpt;
use tokio::{self, io, net};
use tokio_util::codec::{Decoder, FramedRead};

use net_serial_console::*;

const LINE_WRAP: usize = 80;
const TEXT_PLAIN: &str = "text/plain; charset=utf-8";
const TEXT_HTML: &str = "text/html; charset=utf-8";
const TEXT_EVENT_STREAM: &str = "text/event-stream; charset=utf-8";

static CFG: Lazy<RwLock<OptsConsoleWeb>> = Lazy::new(|| RwLock::new(Default::default()));
static INDEX: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new(String::new()));

#[derive(TemplateOnce)]
#[template(path = "console.html.stpl", escape = false)]
struct ConsoleHtml {
    title: String,
    event_url: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut opts = OptsConsoleWeb::from_args();
    opts.finish()?;
    start_pgm(&opts.c, "Serial console web");
    {
        // Store our config
        let mut o = CFG.write();
        *o = opts;
    }
    // Initialize index html from template
    let tmpl = ConsoleHtml {
        title: "Console".into(),
        event_url: "/console/client".into(),
    };
    let html = tmpl.render_once()?;
    {
        let mut i = INDEX.write();
        *i = html;
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(async move {
        run_server().await.unwrap();
    });
    rt.shutdown_timeout(time::Duration::new(5, 0));
    info!("Exit.");
    Ok(())
}

async fn run_server() -> Result<(), Box<dyn Error>> {
    let svc = make_service_fn(move |conn: &AddrStream| {
        let addr = conn.remote_addr();
        async move { Ok::<_, Infallible>(service_fn(move |req| req_router(addr, req))) }
    });
    let listen = CFG.read().listen.clone();
    info!("Listening on {}", &listen);
    let addr = &listen.parse()?;
    let _srv = Server::bind(addr).serve(svc).await?;
    Ok(())
}

async fn req_router(addr: SocketAddr, req: Request<Body>) -> hyper::Result<Response<Body>> {
    info!("{} {} {}", addr, req.method(), req.uri().path());
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") | (&Method::GET, "/console/") => index(req).await,
        (&Method::GET, "/client/") | (&Method::GET, "/console/client/") => client(req).await,
        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header("Content-Type", TEXT_PLAIN)
            .body("Not Found".into())
            .unwrap()),
    }
}

async fn index(_req: Request<Body>) -> hyper::Result<Response<Body>> {
    trace!("in index()");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", TEXT_HTML)
        .body(INDEX.read().clone().into())
        .unwrap())
}

async fn client(_req: Request<Body>) -> hyper::Result<Response<Body>> {
    trace!("in client()");
    let addr;
    {
        addr = CFG.read().connect.clone();
    }
    let conn = net::TcpStream::connect(addr).await;
    if let Err(e) = conn {
        return int_err(format!("Console connection error: {}", e));
    }

    let event_codec = EventCodec::new();
    let event_stream = FramedRead::new(conn.unwrap(), event_codec);
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", TEXT_EVENT_STREAM)
        .header("Cache-Control", "no-cache")
        .body(Body::wrap_stream(event_stream))
        .unwrap())
}

fn int_err(e: String) -> hyper::Result<Response<Body>> {
    Ok(Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header("Content-Type", TEXT_PLAIN)
        .body(e.into())
        .unwrap())
}

struct EventCodec {
    next_index: usize,
    id: u64,
}

impl EventCodec {
    pub fn new() -> EventCodec {
        EventCodec {
            next_index: 0,
            id: 0,
        }
    }
}

impl Decoder for EventCodec {
    type Item = String;
    type Error = tokio::io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<String>, io::Error> {
        let read_to = buf.len();
        let newline_offset = buf[self.next_index..read_to]
            .iter()
            .position(|b| *b == b'\n');

        match newline_offset {
            Some(offset) => {
                let newline_index = self.next_index + offset;
                let bline;
                let line_out;
                if newline_index < LINE_WRAP {
                    bline = buf.split_to(newline_index + 1);
                    let line = &bline[..bline.len() - 1];
                    line_out = without_carriage_return(line);
                    self.next_index = 0;
                    if line_out.is_empty() {
                        return Ok(None);
                    }
                } else {
                    bline = buf.split_to(LINE_WRAP);
                    line_out = &bline[..LINE_WRAP];
                    self.next_index = 0;
                }
                self.id += 1;
                let s = &String::from_utf8_lossy(line_out);
                let s_printable = s.replace(|c: char| !is_printable_ascii(c), "_");
                let ev = format!(
                    "retry: 999999\r\nid: {}\r\ndata: {}\r\n\r\n",
                    self.id, s_printable
                );
                debug!("id {} data: {}", self.id, s_printable);
                Ok(Some(ev))
            }
            None => {
                self.next_index = read_to;
                Ok(None)
            }
        }
    }
}

fn without_carriage_return(s: &[u8]) -> &[u8] {
    if let Some(&b'\r') = s.last() {
        &s[..s.len() - 1]
    } else {
        s
    }
}

fn is_printable_ascii(c: char) -> bool {
    let cu = c as u32;
    cu > 0x1F && cu < 0x7F
}
// EOF
