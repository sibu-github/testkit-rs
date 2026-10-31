use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::{Request, Response};
use hyper::{body::Incoming, service::service_fn};
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

use crate::http::HttpRequest;
use crate::http::expectations::HttpExpectation;
use crate::http::state::{RequestKey, State};

pub(crate) async fn serve(listener: TcpListener, state: Arc<Mutex<State>>) {
    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(handle_stream(stream, state.clone()));
    }
}

pub(crate) async fn serve_tls(
    listener: TcpListener,
    acceptor: TlsAcceptor,
    state: Arc<Mutex<State>>,
) {
    while let Ok((stream, _)) = listener.accept().await {
        let acceptor = acceptor.clone();
        let state = state.clone();
        tokio::spawn(async move {
            if let Ok(stream) = acceptor.accept(stream).await {
                handle_stream(stream, state).await;
            }
        });
    }
}

async fn handle_stream<S>(stream: S, state: Arc<Mutex<State>>)
where
    S: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static,
{
    let io = TokioIo::new(stream);
    let service = service_fn(move |req| handle_request(req, state.clone()));
    if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
        eprintln!("connection error: {e:?}");
    }
}

async fn handle_request(
    req: Request<Incoming>,
    state: Arc<Mutex<State>>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let req = get_http_request(req)
        .await
        .inspect_err(|e| eprintln!("{e:?}"))
        .unwrap();
    let request_key = (req.method.clone(), req.path.clone());
    let expectation = get_expectation(state, &request_key)
        .inspect_err(|e| eprintln!("{e:?}"))
        .unwrap();
    let res = expectation
        .validate_and_respond(&req)
        .await
        .inspect_err(|e| eprintln!("{e:?}"))
        .unwrap();
    Ok(res.into())
}

async fn get_http_request(req: Request<Incoming>) -> anyhow::Result<HttpRequest> {
    let (parts, body) = req.into_parts();
    let method = parts.method.clone();
    let uri = parts.uri.clone();
    let path = uri.path().to_owned();
    let query = uri
        .query()
        .map(|query| {
            url::form_urlencoded::parse(query.as_bytes())
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();
    let headers = parts.headers.clone();
    let body = body.collect().await?.to_bytes();
    let req = HttpRequest {
        method,
        path,
        query,
        headers,
        body,
    };
    Ok(req)
}

fn get_expectation(state: Arc<Mutex<State>>, key: &RequestKey) -> anyhow::Result<HttpExpectation> {
    let expectation = state
        .lock()
        .map_err(|e| anyhow!("{e}"))?
        .get_expetation(key)
        .ok_or_else(|| anyhow!("expectation not set for: {}:{}", key.0, key.1))?;
    Ok(expectation)
}
