// console-web.rs

use std::{net::SocketAddr, sync::Arc, time};

use axum::{
    body::Body,
    extract::State,
    http::{header, Response, StatusCode},
    response::IntoResponse,
    routing::*,
};
use clap::Parser;
use sailfish::TemplateOnce;
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
    let mut opts = OptsConsoleWeb::parse();
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

async fn run_server(opts: OptsConsoleWeb) -> anyhow::Result<()> {
    // Initialize index html from template
    let tmpl = ConsoleHtml {
        title: "Console".into(),
        event_url: "/console/client".into(),
    };
    let index_html = tmpl.render_once()?;
    let ctx = AppCtx {
        connect: Arc::new(opts.connect),
        index: Arc::new(index_html),
    };
    let shared_ctx = Arc::new(ctx);


    let addr = opts.listen.parse::<SocketAddr>()?;
    info!("Listening on {addr}");

    let app = Router::new()
        .route("/", get(index))
        .route("/console/", get(index))
        .route("/client", get(client))
        .route("/console/client", get(client))
        .with_state(shared_ctx);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    Ok(axum::serve(listener, app.into_make_service()).await?)
}


async fn index(State(ctx): State<Arc<AppCtx>>) -> Response<Body> {
    trace!("in index()");


    (StatusCode::OK,
     [(header::CONTENT_TYPE, TEXT_HTML)],
     ctx.index.as_ref().to_string()).into_response()
}

async fn client(State(ctx): State<Arc<AppCtx>>) -> Response<Body> {
    trace!("in client()");
    let conn = match net::TcpStream::connect(ctx.connect.as_ref()).await {
        Err(e) => {
            return err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Console connection error: {e:?}"),
            );
        }
        Ok(c) => c,
    };

    let event_codec = event::EventCodec::new();
    let event_stream = FramedRead::new(conn, event_codec);

    (StatusCode::OK,
     [(header::CONTENT_TYPE, TEXT_EVENT_STREAM), (header::CACHE_CONTROL, "no-cache")],
     Body::from_stream(event_stream)
    ).into_response()
}

fn err_response(code: StatusCode, errmsg: String) -> Response<Body> {
    (
        code,
        [(header::CONTENT_TYPE, TEXT_PLAIN)],
        errmsg
    ).into_response()
}
// EOF
