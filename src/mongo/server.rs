use std::{
    io::Cursor,
    sync::{Arc, Mutex},
};

use anyhow::anyhow;
use bson::{Bson, Document};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

use crate::mongo::{
    command_name::CommandName,
    response::{Response, default_response, handshake_command_response},
    state::State,
};

const HEADER_SIZE: usize = 16;
const OP_MSG: i32 = 2013;
const CHECKSUM_PRESENT: u32 = 1;
const MORE_TO_COME: u32 = 1 << 1;

pub(crate) async fn serve(listener: TcpListener, state: Arc<Mutex<State>>) {
    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(handle_stream(stream, state.clone()));
    }
}

async fn handle_stream(mut stream: TcpStream, state: Arc<Mutex<State>>) {
    loop {
        let mut header = [0u8; HEADER_SIZE];
        if stream.read_exact(&mut header).await.is_err() {
            return;
        }
        let len = i32::from_le_bytes(header[0..4].try_into().unwrap()) as usize;
        let request_id = i32::from_le_bytes(header[4..8].try_into().unwrap());
        let op_code = i32::from_le_bytes(header[12..16].try_into().unwrap());
        assert_eq!(op_code, OP_MSG, "FakeMongo only accept OP_MSG");
        let mut body = vec![0u8; len - HEADER_SIZE];
        if stream.read_exact(&mut body).await.is_err() {
            return;
        }
        let flags = u32::from_le_bytes(body[0..4].try_into().unwrap());
        if flags & MORE_TO_COME != 0 {
            return;
        }
        let res = match handle_stream_inner(&body, flags, state.clone()).await {
            Ok(res) => res,
            Err(e) => {
                eprintln!("Error occurred: {e:?}");
                Response::Error {
                    code: 1,
                    message: e.to_string(),
                }
                .to_bson_doc("")
            }
        };
        let message = prepare_message(res, request_id).unwrap();
        if stream.write_all(&message).await.is_err() {
            return;
        }
    }
}

async fn handle_stream_inner(
    body: &[u8],
    flags: u32,
    state: Arc<Mutex<State>>,
) -> anyhow::Result<Document> {
    let command = parse_sections(&body[4..], flags);
    let response = respond(state.clone(), &command)?;
    Ok(response)
}

/// Merge the kind-0 body and any kind-1 document sequences into one command document
fn parse_sections(mut bytes: &[u8], flags: u32) -> Document {
    if flags & CHECKSUM_PRESENT != 0 {
        bytes = &bytes[..bytes.len() - 4];
    }
    let mut command = Document::new();
    let mut sequences = Vec::new();
    while !bytes.is_empty() {
        let kind = bytes[0];
        bytes = &bytes[1..];
        match kind {
            0 => {
                let size = i32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
                command = Document::from_reader(&mut Cursor::new(&bytes[..size])).unwrap();
                bytes = &bytes[size..];
            }
            1 => {
                let size = i32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
                let section = &bytes[4..size];
                let nul = section.iter().position(|b| *b == 0).unwrap();
                let identifier = String::from_utf8(section[..nul].to_vec()).unwrap();
                let mut docs = Cursor::new(&section[nul + 1..]);
                let mut items = Vec::new();
                while (docs.position() as usize) < section.len() - nul - 1 {
                    items.push(Bson::Document(Document::from_reader(&mut docs).unwrap()));
                }
                sequences.push((identifier, items));
                bytes = &bytes[size..];
            }
            other => panic!("unsupported OP_MSG section kind {other}"),
        }
    }
    for (identifier, items) in sequences {
        command.insert(identifier, items);
    }
    command
}

fn respond(state: Arc<Mutex<State>>, command: &Document) -> anyhow::Result<Document> {
    let Ok(command_name) = CommandName::try_from(command) else {
        eprintln!("Unable to extract command name: {command}");
        return Ok(default_response());
    };
    if command_name == CommandName::Unknown {
        eprintln!("Unknown command received: {command}");
        return Ok(default_response());
    }
    if command_name == CommandName::Handshake {
        return Ok(handshake_command_response());
    }
    let expectation = state
        .lock()
        .map_err(|e| anyhow!("{e}"))?
        .get_expetation(command_name)
        .ok_or_else(|| anyhow!("expectation not set for: {command_name}"))?;
    let response = expectation.validate_and_respond(command)?;
    Ok(response)
}

fn prepare_message(response: Document, request_id: i32) -> anyhow::Result<Vec<u8>> {
    let mut payload = Vec::new();
    response.to_writer(&mut payload)?;
    let mut message = Vec::with_capacity(21 + payload.len());
    message.extend_from_slice(&((21 + payload.len()) as i32).to_le_bytes());
    message.extend_from_slice(&0i32.to_le_bytes());
    message.extend_from_slice(&request_id.to_le_bytes());
    message.extend_from_slice(&OP_MSG.to_le_bytes());
    message.extend_from_slice(&0u32.to_le_bytes());
    message.push(0);
    message.extend_from_slice(&payload);
    Ok(message)
}
