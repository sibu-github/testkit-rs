use std::{
    convert::Infallible,
    sync::{Arc, Mutex},
};

use http_body_util::Full;
use hyper::{
    Method, Request, Response, StatusCode,
    body::{Bytes, Incoming},
    header::{CONTENT_TYPE, HeaderValue},
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use tokio::net::{TcpListener, TcpStream};

use crate::vault::state::State;

pub(crate) async fn serve(listener: TcpListener, state: Arc<Mutex<State>>) {
    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(handle_stream(stream, state.clone()));
    }
}

async fn handle_stream(stream: TcpStream, state: Arc<Mutex<State>>) {
    let io = TokioIo::new(stream);
    let service = service_fn(move |req| handle_request(req, state.clone()));
    if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
        eprintln!("FakeVault HTTP connection error: {e:?}");
    }
}

enum Outcome {
    Found(Bytes),
    NotFound,
    UnknownRoute(String),
    Unsupported,
}

async fn handle_request(
    request: Request<Incoming>,
    state: Arc<Mutex<State>>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let list_method = Method::from_bytes(b"LIST").unwrap();

    // Serialize while holding the lock, then release it before building the
    // response so nothing can fail with the lock held.
    let outcome = {
        let state = state.lock().unwrap();
        let mount = state.vault_mount.as_str();
        match kv2_section(&path, mount) {
            None => Outcome::UnknownRoute(mount.to_owned()),
            Some("metadata") if method == list_method => to_json(state.get_list_response(&path)),
            Some("metadata") if method == Method::GET => {
                to_json(state.get_metadata_response(&path))
            }
            Some("data") if method == Method::GET => to_json(state.get_read_response(&path)),
            Some(_) => Outcome::Unsupported,
        }
    };

    let response = match outcome {
        Outcome::Found(body) => json_response(StatusCode::OK, body),
        // Vault answers a missing secret or empty listing with 404 and an
        // empty error list; vaultrs reports it as `ClientError::APIError`.
        Outcome::NotFound => errors_response(StatusCode::NOT_FOUND, &[]),
        Outcome::UnknownRoute(mount) => {
            let message = format!(
                "FakeVault: no handler for route `{path}`; the fake serves the KV v2 \
                 mount `{mount}`"
            );
            errors_response(StatusCode::NOT_FOUND, &[message])
        }
        Outcome::Unsupported => {
            let message = format!(
                "FakeVault supports only KV v2 read, list, and read-metadata requests; \
                 received {method} {path}"
            );
            errors_response(StatusCode::METHOD_NOT_ALLOWED, &[message])
        }
    };
    Ok(response)
}

/// Returns the KV v2 section (`data` or `metadata`) addressed by `path`, or
/// `None` when the path is not under `/v1/{mount}/`.
fn kv2_section<'a>(path: &'a str, mount: &str) -> Option<&'a str> {
    let rest = path
        .strip_prefix("/v1/")?
        .strip_prefix(mount)?
        .strip_prefix('/')?;
    rest.split('/').next()
}

fn to_json<T: Serialize>(response: Option<&T>) -> Outcome {
    match response {
        Some(response) => Outcome::Found(Bytes::from(serde_json::to_vec(response).unwrap())),
        None => Outcome::NotFound,
    }
}

fn errors_response(status: StatusCode, errors: &[String]) -> Response<Full<Bytes>> {
    let body = serde_json::to_vec(&serde_json::json!({ "errors": errors })).unwrap();
    json_response(status, Bytes::from(body))
}

fn json_response(status: StatusCode, body: Bytes) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .body(Full::new(body))
        .unwrap()
}
