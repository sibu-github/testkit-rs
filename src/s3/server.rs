use std::{
    convert::Infallible,
    sync::{Arc, Mutex},
};

use http_body_util::Full;
use hyper::{
    Method, Request, Response, StatusCode,
    body::{Bytes, Incoming},
    header,
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use tokio::net::{TcpListener, TcpStream};

use crate::s3::state::State;

pub(crate) async fn serve(listener: TcpListener, state: Arc<Mutex<State>>) {
    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(handle_stream(stream, state.clone()));
    }
}

async fn handle_stream(stream: TcpStream, state: Arc<Mutex<State>>) {
    let io = TokioIo::new(stream);
    let service = service_fn(move |req| handle_request(req, state.clone()));
    if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
        eprintln!("FakeSsm HTTP connection error: {e:?}");
    }
}

async fn handle_request(
    request: Request<Incoming>,
    state: Arc<Mutex<State>>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    if let Some(path) = is_get_object_req(&request) {
        let state = state.lock().unwrap();
        let body = state
            .get_objects
            .get(&path)
            .cloned()
            .expect("path is not registered");
        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header(header::CONTENT_LENGTH, body.len())
            .body(Full::new(body))
            .unwrap();
        return Ok(response);
    }

    if let Some(path) = is_put_object_req(&request) {
        let state = state.lock().unwrap();
        state
            .put_objects
            .iter()
            .find(|&v| v == &path)
            .expect("path is not registered");
        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_LENGTH, "0")
            .body(Full::new(Bytes::new()))
            .unwrap();
        return Ok(response);
    }

    unimplemented!()
}

fn is_put_object_req(request: &Request<Incoming>) -> Option<String> {
    if request.method() != Method::PUT {
        return None;
    }
    let uri = request.uri();
    let is_put_object = url::form_urlencoded::parse(uri.query()?.as_bytes())
        .any(|(key, val)| key == "x-id" && val == "PutObject");
    is_put_object.then(|| uri.path().to_owned())
}

fn is_get_object_req(request: &Request<Incoming>) -> Option<String> {
    if request.method() != Method::GET {
        return None;
    }
    let uri = request.uri();
    let is_get_object = url::form_urlencoded::parse(uri.query()?.as_bytes())
        .any(|(key, val)| key == "x-id" && val == "GetObject");
    is_get_object.then(|| uri.path().to_owned())
}
