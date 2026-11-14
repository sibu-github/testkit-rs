use std::{
    convert::Infallible,
    sync::{Arc, Mutex},
};

use http_body_util::{BodyExt, Full};
use hyper::{
    Method, Request, Response, StatusCode,
    body::{Bytes, Incoming},
    header,
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use percent_encoding::percent_decode_str;
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
        eprintln!("FakeS3 HTTP connection error: {e:?}");
    }
}

async fn handle_request(
    request: Request<Incoming>,
    state: Arc<Mutex<State>>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    if let Some(path) = is_get_object_req(&request) {
        let body = state.lock().unwrap().get_objects.get(&path).cloned();
        let Some(body) = body else {
            return Ok(error_response(
                StatusCode::NOT_FOUND,
                "NoSuchKey",
                "The specified key does not exist.",
                &path,
            ));
        };
        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header(header::CONTENT_LENGTH, body.len())
            .body(Full::new(body))
            .unwrap();
        return Ok(response);
    }

    if let Some(path) = is_put_object_req(&request) {
        if let Err(e) = request.into_body().collect().await {
            let message = format!("FakeS3 could not read the PutObject body: {e}");
            return Ok(error_response(
                StatusCode::BAD_REQUEST,
                "IncompleteBody",
                &message,
                &path,
            ));
        }
        let is_registered = state.lock().unwrap().put_objects.contains(&path);
        if !is_registered {
            let message = format!(
                "FakeS3 received a PutObject request for `{path}`, which was not \
                 registered with FakeS3::put_object"
            );
            return Ok(error_response(
                StatusCode::BAD_REQUEST,
                "UnexpectedRequest",
                &message,
                &path,
            ));
        }
        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_LENGTH, "0")
            .body(Full::new(Bytes::new()))
            .unwrap();
        return Ok(response);
    }

    let message = format!(
        "FakeS3 supports only GetObject and PutObject; received {} {}",
        request.method(),
        request.uri()
    );
    Ok(error_response(
        StatusCode::NOT_IMPLEMENTED,
        "NotImplemented",
        &message,
        request.uri().path(),
    ))
}

/// Builds an S3-style XML error response that the AWS SDK parses into a typed
/// error, such as `GetObjectError::NoSuchKey` for the `NoSuchKey` code.
fn error_response(
    status: StatusCode,
    code: &str,
    message: &str,
    resource: &str,
) -> Response<Full<Bytes>> {
    let body = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <Error><Code>{}</Code><Message>{}</Message><Resource>{}</Resource>\
         <RequestId>fake-s3</RequestId></Error>",
        xml_escape(code),
        xml_escape(message),
        xml_escape(resource),
    );
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/xml")
        .header(header::CONTENT_LENGTH, body.len())
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Returns the request path with percent-encoding removed, so it can be
/// compared with the `/{bucket}/{key}` paths registered on [`State`]. The SDK
/// percent-encodes object keys, for example a space becomes `%20`.
fn decoded_path(request: &Request<Incoming>) -> String {
    percent_decode_str(request.uri().path())
        .decode_utf8_lossy()
        .into_owned()
}

fn is_put_object_req(request: &Request<Incoming>) -> Option<String> {
    if request.method() != Method::PUT {
        return None;
    }
    let uri = request.uri();
    let is_put_object = url::form_urlencoded::parse(uri.query()?.as_bytes())
        .any(|(key, val)| key == "x-id" && val == "PutObject");
    is_put_object.then(|| decoded_path(request))
}

fn is_get_object_req(request: &Request<Incoming>) -> Option<String> {
    if request.method() != Method::GET {
        return None;
    }
    let uri = request.uri();
    let is_get_object = url::form_urlencoded::parse(uri.query()?.as_bytes())
        .any(|(key, val)| key == "x-id" && val == "GetObject");
    is_get_object.then(|| decoded_path(request))
}
