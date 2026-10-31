//! Per-connection Postgres wire-protocol (v3) handling.

use std::collections::HashMap;
use std::io;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::postgres::state::{Response, State, command_tag};
use crate::postgres::types::encode;

const MAX_MESSAGE: i32 = 64 * 1024 * 1024;

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_owned())
}

/// Appends one backend message (`type`, length, body) to `out`.
fn put(out: &mut Vec<u8>, ty: u8, body: &[u8]) {
    out.push(ty);
    out.extend_from_slice(&((body.len() + 4) as i32).to_be_bytes());
    out.extend_from_slice(body);
}

/// Collapses whitespace and strips trailing semicolons so matching is layout-insensitive.
pub(crate) fn normalize(sql: &str) -> String {
    let joined = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    joined
        .trim_end_matches(|c: char| c == ';' || c.is_whitespace())
        .to_owned()
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }
    fn take(&mut self, n: usize) -> io::Result<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|e| *e <= self.buf.len());
        let end = end.ok_or_else(|| invalid("message too short"))?;
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn i16(&mut self) -> io::Result<i16> {
        Ok(i16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> io::Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn cstr(&mut self) -> io::Result<String> {
        let rest = &self.buf[self.pos..];
        let n = rest
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| invalid("unterminated string"))?;
        let s = String::from_utf8_lossy(&rest[..n]).into_owned();
        self.pos += n + 1;
        Ok(s)
    }
}

/// Highest `$n` placeholder in the SQL (how many parameters the statement takes).
fn param_count(sql: &str) -> usize {
    let bytes = sql.as_bytes();
    let mut max = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let mut j = i + 1;
            let mut n = 0usize;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                n = n
                    .saturating_mul(10)
                    .saturating_add((bytes[j] - b'0') as usize);
                j += 1;
            }
            if j > i + 1 {
                max = max.max(n);
            }
            i = j;
        } else {
            i += 1;
        }
    }
    max.min(i16::MAX as usize)
}

/// Format code for result column `i` given the Bind message's result format codes.
fn format_for(formats: &[i16], i: usize) -> i16 {
    match formats.len() {
        0 => 0,
        1 => formats[0],
        _ => formats.get(i).copied().unwrap_or(0),
    }
}

struct Statement {
    sql: String,
    param_oids: Vec<u32>,
}

struct Portal {
    sql: String,
    params: Vec<Option<Vec<u8>>>,
    result_formats: Vec<i16>,
}

struct Conn {
    state: Arc<State>,
    statements: HashMap<String, Statement>,
    portals: HashMap<String, Portal>,
    out: Vec<u8>,
    /// Transaction status byte for ReadyForQuery: b'I', b'T' or b'E'.
    tx: u8,
    /// After an extended-protocol error, ignore messages until Sync.
    failed: bool,
}

impl Conn {
    fn new(state: Arc<State>) -> Self {
        Self {
            state,
            statements: HashMap::new(),
            portals: HashMap::new(),
            out: Vec::new(),
            tx: b'I',
            failed: false,
        }
    }

    fn error(&mut self, code: &str, message: &str) {
        let mut body = Vec::new();
        for (field, value) in [
            (b'S', "ERROR"),
            (b'V', "ERROR"),
            (b'C', code),
            (b'M', message),
        ] {
            body.push(field);
            body.extend_from_slice(value.as_bytes());
            body.push(0);
        }
        body.push(0);
        put(&mut self.out, b'E', &body);
        if self.tx == b'T' {
            self.tx = b'E';
        }
    }

    fn extended_error(&mut self, code: &str, message: &str) {
        self.error(code, message);
        self.failed = true;
    }

    fn ready(&mut self) {
        let tx = self.tx;
        put(&mut self.out, b'Z', &[tx]);
    }

    fn track_tx(&mut self, sql: &str) {
        let word = sql
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        match word.as_str() {
            "BEGIN" | "START" => self.tx = b'T',
            "COMMIT" | "END" | "ROLLBACK" | "ABORT" => self.tx = b'I',
            _ => {}
        }
    }

    fn row_description(&mut self, response: &Response, formats: &[i16]) {
        match response {
            Response::Rows { columns, .. } => {
                let mut body = (columns.len() as i16).to_be_bytes().to_vec();
                for (i, col) in columns.iter().enumerate() {
                    body.extend_from_slice(col.name.as_bytes());
                    body.push(0);
                    body.extend_from_slice(&0i32.to_be_bytes()); // table oid
                    body.extend_from_slice(&0i16.to_be_bytes()); // column attr number
                    body.extend_from_slice(&col.ty.oid().to_be_bytes());
                    body.extend_from_slice(&col.ty.size().to_be_bytes());
                    body.extend_from_slice(&(-1i32).to_be_bytes()); // type modifier
                    body.extend_from_slice(&format_for(formats, i).to_be_bytes());
                }
                put(&mut self.out, b'T', &body);
            }
            _ => put(&mut self.out, b'n', &[]), // NoData
        }
    }

    /// Runs one statement: records it, resolves the mock, emits the response messages.
    /// Returns false if an ErrorResponse was emitted.
    fn run_statement(
        &mut self,
        sql: &str,
        params: &[Option<Vec<u8>>],
        formats: &[i16],
        send_row_description: bool,
    ) -> bool {
        let sql = normalize(sql);
        if sql.is_empty() {
            put(&mut self.out, b'I', &[]); // EmptyQueryResponse
            return true;
        }
        self.state.record(&sql, params);

        match self.state.resolve(&sql) {
            Response::Error { code, message } => {
                self.error(&code, &message);
                return false;
            }
            Response::Command(tag) => put(&mut self.out, b'C', &cstring(&tag)),
            Response::Rows { columns, rows } => {
                let mut data_rows = Vec::with_capacity(rows.len());
                for row in &rows {
                    if row.len() != columns.len() {
                        let msg = format!(
                            "FakePostgres: row has {} values but {} columns were declared",
                            row.len(),
                            columns.len()
                        );
                        self.error("XX000", &msg);
                        return false;
                    }
                    let mut body = (columns.len() as i16).to_be_bytes().to_vec();
                    for (i, (col, value)) in columns.iter().zip(row).enumerate() {
                        let binary = format_for(formats, i) == 1;
                        match encode(col.ty, value, binary) {
                            Ok(None) => body.extend_from_slice(&(-1i32).to_be_bytes()),
                            Ok(Some(bytes)) => {
                                body.extend_from_slice(&(bytes.len() as i32).to_be_bytes());
                                body.extend_from_slice(&bytes);
                            }
                            Err(msg) => {
                                self.error("XX000", &msg);
                                return false;
                            }
                        }
                    }
                    data_rows.push(body);
                }
                if send_row_description {
                    self.row_description(
                        &Response::Rows {
                            columns,
                            rows: vec![],
                        },
                        formats,
                    );
                }
                let count = data_rows.len();
                for body in data_rows {
                    put(&mut self.out, b'D', &body);
                }
                put(&mut self.out, b'C', &cstring(&command_tag(&sql, count)));
            }
        }
        self.track_tx(&sql);
        true
    }

    // ---- simple query protocol ----

    fn handle_query(&mut self, body: &[u8]) -> io::Result<()> {
        let sql = Reader::new(body).cstr()?;
        let whole = normalize(&sql);
        if whole.is_empty() {
            put(&mut self.out, b'I', &[]);
        } else {
            // A mock for the whole string wins; otherwise split multi-statement strings.
            let parts: Vec<String> = if !whole.contains(';') || self.state.has_match(&whole) {
                vec![whole]
            } else {
                sql.split(';')
                    .map(normalize)
                    .filter(|s| !s.is_empty())
                    .collect()
            };
            for part in parts {
                if !self.run_statement(&part, &[], &[], true) {
                    break;
                }
            }
        }
        self.ready();
        Ok(())
    }

    // ---- extended query protocol ----

    fn handle_parse(&mut self, body: &[u8]) -> io::Result<()> {
        let mut r = Reader::new(body);
        let name = r.cstr()?;
        let sql = r.cstr()?;
        let n = r.i16()?.max(0) as usize;
        let mut param_oids = Vec::with_capacity(n);
        for _ in 0..n {
            param_oids.push(r.i32()? as u32);
        }
        let needed = param_count(&sql);
        if param_oids.len() < needed {
            param_oids.resize(needed, 0);
        }
        self.statements.insert(name, Statement { sql, param_oids });
        put(&mut self.out, b'1', &[]); // ParseComplete
        Ok(())
    }

    fn handle_bind(&mut self, body: &[u8]) -> io::Result<()> {
        let mut r = Reader::new(body);
        let portal = r.cstr()?;
        let statement = r.cstr()?;
        for _ in 0..r.i16()?.max(0) {
            r.i16()?; // parameter format codes (raw bytes are kept as-is)
        }
        let mut params = Vec::new();
        for _ in 0..r.i16()?.max(0) {
            let len = r.i32()?;
            params.push(if len < 0 {
                None
            } else {
                Some(r.take(len as usize)?.to_vec())
            });
        }
        let mut result_formats = Vec::new();
        for _ in 0..r.i16()?.max(0) {
            result_formats.push(r.i16()?);
        }
        let Some(sql) = self.statements.get(&statement).map(|s| s.sql.clone()) else {
            self.extended_error(
                "26000",
                &format!("prepared statement \"{statement}\" does not exist"),
            );
            return Ok(());
        };
        self.portals.insert(
            portal,
            Portal {
                sql,
                params,
                result_formats,
            },
        );
        put(&mut self.out, b'2', &[]); // BindComplete
        Ok(())
    }

    fn handle_describe(&mut self, body: &[u8]) -> io::Result<()> {
        let mut r = Reader::new(body);
        let kind = r.u8()?;
        let name = r.cstr()?;
        match kind {
            b'S' => {
                let Some((sql, oids)) = self
                    .statements
                    .get(&name)
                    .map(|s| (s.sql.clone(), s.param_oids.clone()))
                else {
                    self.extended_error(
                        "26000",
                        &format!("prepared statement \"{name}\" does not exist"),
                    );
                    return Ok(());
                };
                let mut desc = (oids.len() as i16).to_be_bytes().to_vec();
                for oid in oids {
                    desc.extend_from_slice(&oid.to_be_bytes());
                }
                put(&mut self.out, b't', &desc); // ParameterDescription
                let response = self.state.resolve(&normalize(&sql));
                self.row_description(&response, &[]);
            }
            _ => {
                let Some((sql, formats)) = self
                    .portals
                    .get(&name)
                    .map(|p| (p.sql.clone(), p.result_formats.clone()))
                else {
                    self.extended_error("34000", &format!("portal \"{name}\" does not exist"));
                    return Ok(());
                };
                let response = self.state.resolve(&normalize(&sql));
                self.row_description(&response, &formats);
            }
        }
        Ok(())
    }

    fn handle_execute(&mut self, body: &[u8]) -> io::Result<()> {
        let name = Reader::new(body).cstr()?;
        let Some((sql, params, formats)) = self
            .portals
            .get(&name)
            .map(|p| (p.sql.clone(), p.params.clone(), p.result_formats.clone()))
        else {
            self.extended_error("34000", &format!("portal \"{name}\" does not exist"));
            return Ok(());
        };
        if !self.run_statement(&sql, &params, &formats, false) {
            self.failed = true;
        }
        Ok(())
    }

    fn handle_close(&mut self, body: &[u8]) -> io::Result<()> {
        let mut r = Reader::new(body);
        let kind = r.u8()?;
        let name = r.cstr()?;
        if kind == b'S' {
            self.statements.remove(&name);
        } else {
            self.portals.remove(&name);
        }
        put(&mut self.out, b'3', &[]); // CloseComplete
        Ok(())
    }
}

fn cstring(s: &str) -> Vec<u8> {
    let mut v = s.as_bytes().to_vec();
    v.push(0);
    v
}

async fn read_startup(sock: &mut TcpStream) -> io::Result<Vec<u8>> {
    let len = sock.read_i32().await?;
    if !(8..=MAX_MESSAGE).contains(&len) {
        return Err(invalid("bad startup length"));
    }
    let mut body = vec![0u8; (len - 4) as usize];
    sock.read_exact(&mut body).await?;
    Ok(body)
}

async fn read_message(sock: &mut TcpStream) -> io::Result<(u8, Vec<u8>)> {
    let ty = sock.read_u8().await?;
    let len = sock.read_i32().await?;
    if !(4..=MAX_MESSAGE).contains(&len) {
        return Err(invalid("bad message length"));
    }
    let mut body = vec![0u8; (len - 4) as usize];
    sock.read_exact(&mut body).await?;
    Ok((ty, body))
}

/// Serves one client connection until it terminates or errors.
pub(crate) async fn serve(mut sock: TcpStream, state: Arc<State>) {
    serve_inner(&mut sock, state).await.unwrap();
}

async fn serve_inner(sock: &mut TcpStream, state: Arc<State>) -> io::Result<()> {
    // Startup: decline SSL/GSS encryption, then accept the StartupMessage without auth.
    loop {
        let body = read_startup(sock).await?;
        match i32::from_be_bytes(body[0..4].try_into().unwrap()) {
            80877103 | 80877104 => sock.write_all(b"N").await?, // SSLRequest / GSSENCRequest
            80877102 => return Ok(()),                          // CancelRequest
            _ => break,
        }
    }

    let mut conn = Conn::new(state);
    put(&mut conn.out, b'R', &0i32.to_be_bytes()); // AuthenticationOk
    for (k, v) in [
        ("server_version", "16.0"),
        ("server_encoding", "UTF8"),
        ("client_encoding", "UTF8"),
        ("DateStyle", "ISO, MDY"),
        ("integer_datetimes", "on"),
        ("standard_conforming_strings", "on"),
        ("TimeZone", "UTC"),
    ] {
        let mut body = cstring(k);
        body.extend_from_slice(&cstring(v));
        put(&mut conn.out, b'S', &body); // ParameterStatus
    }
    let mut key = 4242i32.to_be_bytes().to_vec(); // BackendKeyData: pid + secret
    key.extend_from_slice(&4242i32.to_be_bytes());
    put(&mut conn.out, b'K', &key);
    conn.ready();
    sock.write_all(&conn.out).await?;
    conn.out.clear();

    loop {
        let (ty, body) = read_message(sock).await?;
        if conn.failed && ty != b'S' && ty != b'X' {
            continue; // discard everything until Sync after an error
        }
        match ty {
            b'X' => return Ok(()), // Terminate
            b'Q' => conn.handle_query(&body)?,
            b'P' => conn.handle_parse(&body)?,
            b'B' => conn.handle_bind(&body)?,
            b'D' => conn.handle_describe(&body)?,
            b'E' => conn.handle_execute(&body)?,
            b'C' => conn.handle_close(&body)?,
            b'S' => {
                conn.failed = false;
                conn.ready();
            }
            b'H' => {} // Flush: nothing extra, we flush below
            _ => {}    // anything else (COPY data, FunctionCall, ...) is ignored
        }
        if matches!(ty, b'Q' | b'S' | b'H') && !conn.out.is_empty() {
            sock.write_all(&conn.out).await?;
            conn.out.clear();
        }
    }
}
