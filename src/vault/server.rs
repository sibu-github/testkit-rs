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

async fn handle_request(
    request: Request<Incoming>,
    state: Arc<Mutex<State>>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let method = request.method().clone();
    let uri = request.uri();
    let path = uri.path();
    let list_method = Method::from_bytes(b"LIST").unwrap();
    if method == list_method {
        let state = state.lock().unwrap();
        let response = state.get_list_response(path).unwrap();
        return json_response(response);
    }
    if method != Method::GET {
        unimplemented!()
    }
    let vault_mount = state.lock().unwrap().vault_mount();
    if is_metadata_req(path, &vault_mount) {
        let state = state.lock().unwrap();
        let response = state.get_metadata_response(path).unwrap();
        return json_response(response);
    }
    if is_data_req(path, &vault_mount) {
        let state = state.lock().unwrap();
        let response = state.get_read_response(path).unwrap();
        return json_response(response);
    }

    unimplemented!()
}

fn is_metadata_req(path: &str, vault_mount: &str) -> bool {
    let path = path.strip_prefix("/v1/").unwrap();
    let path = path.strip_prefix(vault_mount).unwrap();
    let path = path.strip_prefix('/').unwrap();
    path.starts_with("metadata")
}

fn is_data_req(path: &str, vault_mount: &str) -> bool {
    let path = path.strip_prefix("/v1/").unwrap();
    let path = path.strip_prefix(vault_mount).unwrap();
    let path = path.strip_prefix('/').unwrap();
    path.starts_with("data")
}

fn json_response<T>(body: &T) -> Result<Response<Full<Bytes>>, Infallible>
where
    T: Serialize,
{
    let body = Bytes::from(serde_json::to_vec(body).unwrap());
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .body(Full::new(body))
        .unwrap();
    Ok(response)
}
