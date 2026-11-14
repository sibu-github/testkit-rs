use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex, PoisonError};

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::header::{CONTENT_TYPE, HeaderValue};
use hyper::server::conn::http1;
use hyper::{Request, Response, StatusCode};
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
    let req = match get_http_request(req).await {
        Ok(req) => req,
        Err(e) => {
            let message = format!("FakeHttp could not read the request: {e:#}");
            return Ok(failure(StatusCode::INTERNAL_SERVER_ERROR, message));
        }
    };
    let (method, path) = (req.method.clone(), req.path.clone());
    let Some(expectation) = get_expectation(&state, &(method.clone(), path.clone())) else {
        let message = format!("FakeHttp has no expectation registered for {method} {path}");
        return Ok(failure(StatusCode::NOT_IMPLEMENTED, message));
    };
    match expectation.validate_and_respond(&req).await {
        Ok(res) => Ok(res.into()),
        Err(e) => {
            let message = format!("FakeHttp request {method} {path} did not match: {e:#}");
            Ok(failure(StatusCode::INTERNAL_SERVER_ERROR, message))
        }
    }
}

/// Reports a request the fake could not answer as configured. The message is
/// printed so it appears in the failing test's output, and is also returned as
/// the response body so the client can see it.
fn failure(status: StatusCode, message: String) -> Response<Full<Bytes>> {
    eprintln!("{message}");
    let mut res = Response::new(Full::new(Bytes::from(message)));
    *res.status_mut() = status;
    res.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    res
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

fn get_expectation(state: &Mutex<State>, key: &RequestKey) -> Option<HttpExpectation> {
    state
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_expetation(key)
}
