use std::{
    convert::Infallible,
    sync::{Arc, Mutex},
};

use http_body_util::{BodyExt, Full};
use hyper::{
    Request, Response, StatusCode,
    body::{Bytes, Incoming},
    header::{CONTENT_TYPE, HeaderValue},
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use serde_json::{Value as JsonValue, json};
use tokio::net::{TcpListener, TcpStream};

use crate::ssm::state::State;

const TARGET: &str = "x-amz-target";
const GET_PARAMETER: &str = "GetParameter";
const GET_PARAMETERS: &str = "GetParameters";

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
    let target = get_target(&request);
    let input = match request.into_body().collect().await {
        Ok(body) => serde_json::from_slice::<JsonValue>(&body.to_bytes()),
        Err(e) => {
            let message = format!("FakeSsm could not read the request body: {e}");
            return error_response("ValidationException", &message);
        }
    };
    let Ok(input) = input else {
        return error_response(
            "ValidationException",
            "FakeSsm could not parse the request body as JSON",
        );
    };
    let parameters = state.lock().unwrap().parameters();
    match target.as_str() {
        GET_PARAMETER => {
            let Some(name) = input["Name"].as_str() else {
                return error_response(
                    "ValidationException",
                    "FakeSsm expected a string `Name` in the GetParameter request",
                );
            };
            let Some((_, val)) = parameters.iter().find(|(p, _)| p == name) else {
                let message = format!("FakeSsm has no parameter registered with name `{name}`");
                return error_response("ParameterNotFound", &message);
            };
            let body = json!({ "Parameter": parameter_value(name, val) });
            json_response(&body)
        }
        GET_PARAMETERS => {
            let names = input["Names"].as_array().and_then(|names| {
                names
                    .iter()
                    .map(|name| name.as_str().map(str::to_owned))
                    .collect::<Option<Vec<_>>>()
            });
            let Some(names) = names else {
                return error_response(
                    "ValidationException",
                    "FakeSsm expected a string array `Names` in the GetParameters request",
                );
            };
            let mut valid_parameters = Vec::new();
            let mut invalid_parameters = Vec::new();
            for name in names.iter() {
                if let Some((_, val)) = parameters.iter().find(|(p, _)| p == name) {
                    valid_parameters.push(parameter_value(name, val));
                } else {
                    invalid_parameters.push(name);
                }
            }
            let body = json!({
                "Parameters": valid_parameters,
                "InvalidParameters": invalid_parameters
            });
            json_response(&body)
        }
        other => {
            let message =
                format!("FakeSsm supports only GetParameter and GetParameters; received `{other}`");
            error_response("UnknownOperationException", &message)
        }
    }
}

fn get_target(request: &Request<Incoming>) -> String {
    request
        .headers()
        .get(TARGET)
        .and_then(|value| value.to_str().ok())
        .and_then(|val| val.rsplit('.').next())
        .map(str::to_string)
        .unwrap_or_default()
}

fn parameter_value(name: &str, value: &str) -> JsonValue {
    json!({
        "Name": name,
        "Type": "String",
        "Value": value,
        "Version": 1
    })
}

fn json_response(body: &JsonValue) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(json_response_with_status(StatusCode::OK, body))
}

/// Builds an AWS JSON 1.1 error response. The SDK reads the `__type` field to
/// produce a typed error, such as `GetParameterError::ParameterNotFound`.
/// A 400 status is used so the SDK reports the error without retrying.
fn error_response(error_type: &str, message: &str) -> Result<Response<Full<Bytes>>, Infallible> {
    let body = json!({ "__type": error_type, "message": message });
    Ok(json_response_with_status(StatusCode::BAD_REQUEST, &body))
}

fn json_response_with_status(status: StatusCode, body: &JsonValue) -> Response<Full<Bytes>> {
    let body = Bytes::from(serde_json::to_vec(body).unwrap());
    Response::builder()
        .status(status)
        .header(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-amz-json-1.1"),
        )
        .body(Full::new(body))
        .unwrap()
}
