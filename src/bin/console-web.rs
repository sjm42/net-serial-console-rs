// console-web.rs

use anyhow::anyhow;
use hyper::service::{make_service_fn, service_fn};
use hyper::{http, server::conn::AddrStream, Body, Method, Request, Response, Server, StatusCode};
use log::*;
use sailfish::TemplateOnce;
use std::{convert::Infallible, net::SocketAddr, sync::Arc, time};
use structopt::StructOpt;
use tokio::net;
use tokio_util::codec::FramedRead;

use net_serial_console::*;

const TEXT_HTML: &str = "text/html; charset=utf-8";
const TEXT_PLAIN: &str = "text/plain; charset=utf-8";
const TEXT_EVENT_STREAM: &str = "text/event-stream; charset=utf-8";

#[derive(Clone, TemplateOnce)]
#[template(path = "console.html.stpl", escape = false)]
struct ConsoleHtml {
    title: String,
    event_url: String,
}

#[derive(Clone)]
struct AppCtx {
    connect: Arc<String>,
    index: Arc<String>,
}

fn main() -> anyhow::Result<()> {
    let mut opts = OptsConsoleWeb::from_args();
    opts.finish()?;
    start_pgm(&opts.c, "Serial console web");

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

async fn run_server(opts: OptsConsoleWeb) -> anyhow::Result<()> {
    // Initialize index html from template
    let tmpl = ConsoleHtml {
        title: "Console".into(),
        event_url: "/console/client".into(),
    };
    let index = tmpl.render_once()?;
    let ctx = AppCtx {
        connect: Arc::new(opts.connect),
        index: Arc::new(index),
    };

    let listen = &opts.listen;
    let addr = listen.parse()?;
    info!("Listening on {}", listen);

    let svc = make_service_fn(move |conn: &AddrStream| {
        let ctx_r = ctx.clone();
        let addr = conn.remote_addr();
        async move { Ok::<_, Infallible>(service_fn(move |req| req_router(ctx_r.clone(), addr, req))) }
    });
    let server = Server::bind(&addr).serve(svc);
    if let Err(e) = server.await {
        error!("Server error: {}", e);
        return Err(anyhow!(e));
    }
    Ok(())
}

async fn req_router(
    ctx: AppCtx,
    addr: SocketAddr,
    req: Request<Body>,
) -> http::Result<Response<Body>> {
    info!("{} {} {}", addr, req.method(), req.uri().path());
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") | (&Method::GET, "/console/") => index(&ctx, req).await,
        (&Method::GET, "/client") | (&Method::GET, "/console/client") => client(&ctx, req).await,
        _ => err_response(StatusCode::NOT_FOUND, "Not found".into()),
    }
}

async fn index(ctx: &AppCtx, _req: Request<Body>) -> http::Result<Response<Body>> {
    trace!("in index()");
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", TEXT_HTML)
        .body(ctx.index.as_ref().to_owned().into())
}

async fn client(ctx: &AppCtx, _req: Request<Body>) -> http::Result<Response<Body>> {
    trace!("in client()");
    let conn = net::TcpStream::connect(ctx.connect.as_ref()).await;
    if let Err(e) = conn {
        return err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Console connection error: {}", e),
        );
    }

    let event_codec = event::EventCodec::new();
    let event_stream = FramedRead::new(conn.unwrap(), event_codec);
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", TEXT_EVENT_STREAM)
        .header("Cache-Control", "no-cache")
        .body(Body::wrap_stream(event_stream))
}

fn err_response(code: StatusCode, errmsg: String) -> http::Result<Response<Body>> {
    Response::builder()
        .status(code)
        .header("Content-Type", TEXT_PLAIN)
        .body(errmsg.into())
}
// EOF
