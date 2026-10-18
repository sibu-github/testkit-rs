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
    let body = request.into_body().collect().await.unwrap();
    let input: JsonValue = serde_json::from_slice(&body.to_bytes()).unwrap();
    let parameters = state.lock().unwrap().parameters();
    match target.as_str() {
        GET_PARAMETER => {
            let name = input["Name"].as_str().unwrap().to_owned();
            let (_, val) = parameters.iter().find(|(p, _)| p == &name).unwrap();
            let body = param_value(&name, val);
            json_response(&body)
        }
        GET_PARAMETERS => {
            let names = input["Names"]
                .as_array()
                .unwrap()
                .iter()
                .map(|name| name.as_str().unwrap().to_owned())
                .collect::<Vec<_>>();
            let mut valid_parameters = Vec::new();
            let mut invalid_parameters = Vec::new();
            for name in names.iter() {
                if let Some((_, val)) = parameters.iter().find(|(p, _)| p == name) {
                    valid_parameters.push(param_value(name, val));
                } else {
                    invalid_parameters.push(name);
                }
            }
            let body = json!({
                "Parameters": parameters,
                "InvalidParameters": invalid_parameters
            });
            json_response(&body)
        }
        _ => unreachable!(),
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

fn param_value(name: &str, value: &str) -> JsonValue {
    json!({
        "Parameter": {
            "Name": name,
            "Type": "String",
            "Value": value,
            "Version": 1
        }
    })
}

fn json_response(body: &JsonValue) -> Result<Response<Full<Bytes>>, Infallible> {
    let body = Bytes::from(serde_json::to_vec(body).unwrap());
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-amz-json-1.1"),
        )
        .body(Full::new(body))
        .unwrap();
    Ok(response)
}
