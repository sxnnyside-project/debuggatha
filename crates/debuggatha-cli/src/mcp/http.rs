//! `debuggatha mcp --http`: Streamable HTTP at `/mcp`, stateless like the TypeScript server: every
//! POST carries one JSON-RPC message (or a batch) and is answered with JSON. It binds only to a
//! loopback address, because nothing here signs a caller in, and it refuses a request whose Host
//! or Origin is not local, so a web page cannot reach it by DNS rebinding.
//!
//! The server never asks the client anything over HTTP, so there is no model of the client and no
//! progress; a local model chosen with `--semantic` still works.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use debuggatha_engine::daemon::MAX_LINE_BYTES;
use serde_json::{Value, json};

use crate::mcp::server::{Repositories, Server};

const MAX_HEAD_BYTES: usize = 64 * 1024;
const MAX_CONNECTIONS: usize = 64;
const IDLE: Duration = Duration::from_secs(30);

fn loopback(host: &str) -> bool {
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    bare == "localhost" || bare.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// The host name of a `Host` header or an `Origin`, without scheme, port or brackets.
fn hostname(value: &str) -> &str {
    let rest = value.split_once("://").map_or(value, |(_, r)| r);
    let authority = rest.split('/').next().unwrap_or(rest);
    if let Some(inner) = authority.strip_prefix('[') {
        return inner.split(']').next().unwrap_or(inner);
    }
    authority.split(':').next().unwrap_or(authority)
}

struct Head {
    method: String,
    path: String,
    host: Option<String>,
    origin: Option<String>,
    length: Option<usize>,
    chunked: bool,
}

fn read_head(reader: &mut BufReader<TcpStream>) -> Option<Head> {
    let mut head = Vec::new();
    loop {
        let mut line = Vec::new();
        reader
            .by_ref()
            .take(MAX_HEAD_BYTES as u64)
            .read_until(b'\n', &mut line)
            .ok()?;
        if line.is_empty() || head.len() + line.len() > MAX_HEAD_BYTES {
            return None;
        }
        let blank = line == b"\r\n" || line == b"\n";
        head.extend_from_slice(&line);
        if blank {
            break;
        }
    }
    let text = String::from_utf8_lossy(&head);
    let mut lines = text.lines();
    let mut first = lines.next()?.split_whitespace();
    let (method, path) = (first.next()?.to_owned(), first.next()?.to_owned());
    let mut parsed = Head {
        method,
        path,
        host: None,
        origin: None,
        length: None,
        chunked: false,
    };
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            "host" => parsed.host = Some(value.to_owned()),
            "origin" => parsed.origin = Some(value.to_owned()),
            "content-length" => parsed.length = value.parse().ok(),
            "transfer-encoding" => parsed.chunked = value.to_ascii_lowercase().contains("chunked"),
            _ => {}
        }
    }
    Some(parsed)
}

fn respond(stream: &mut TcpStream, status: u16, reason: &str, extra: &str, body: &str) {
    let message = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",
        body.len()
    );
    // A client that went away has no one to tell.
    drop(stream.write_all(message.as_bytes()));
    drop(stream.flush());
}

fn refuse(stream: &mut TcpStream, status: u16, reason: &str, message: &str) {
    let body =
        json!({ "jsonrpc": "2.0", "error": { "code": -32000, "message": message }, "id": null });
    respond(stream, status, reason, "", &body.to_string());
}

fn connection(stream: TcpStream, server: &Server<'_>) {
    let mut stream = stream;
    stream.set_read_timeout(Some(IDLE)).ok();
    stream.set_write_timeout(Some(IDLE)).ok();
    let Ok(clone) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(clone);
    let Some(head) = read_head(&mut reader) else {
        return refuse(&mut stream, 400, "Bad Request", "Invalid request.");
    };
    if !head.host.as_deref().is_some_and(|h| loopback(hostname(h)))
        || head
            .origin
            .as_deref()
            .is_some_and(|o| !loopback(hostname(o)))
    {
        return refuse(&mut stream, 403, "Forbidden", "Forbidden host.");
    }
    if head.path.split('?').next() != Some("/mcp") {
        return refuse(
            &mut stream,
            404,
            "Not Found",
            "Not found. The MCP endpoint is /mcp.",
        );
    }
    if head.method != "POST" {
        let body = json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32000,
            "message": "Method not allowed. This endpoint is stateless: use POST." } });
        return respond(
            &mut stream,
            405,
            "Method Not Allowed",
            "Allow: POST\r\n",
            &body.to_string(),
        );
    }
    let Some(length) = head.length.filter(|_| !head.chunked) else {
        return refuse(
            &mut stream,
            411,
            "Length Required",
            "A Content-Length is required.",
        );
    };
    if length > MAX_LINE_BYTES {
        return refuse(
            &mut stream,
            413,
            "Payload Too Large",
            "A message is longer than 16 MiB.",
        );
    }
    let mut body = vec![0; length];
    if reader.read_exact(&mut body).is_err() {
        return refuse(&mut stream, 400, "Bad Request", "Invalid request.");
    }
    let Ok(message) = serde_json::from_slice::<Value>(&body) else {
        let error = json!({ "jsonrpc": "2.0", "id": null,
            "error": { "code": -32700, "message": "The message is not JSON." } });
        return respond(&mut stream, 400, "Bad Request", "", &error.to_string());
    };
    let answers: Vec<Value> = match &message {
        Value::Array(batch) => batch.iter().filter_map(|m| answer(server, m)).collect(),
        single => answer(server, single).into_iter().collect(),
    };
    match (&message, answers.as_slice()) {
        (_, []) => respond(&mut stream, 202, "Accepted", "", ""),
        (Value::Array(_), _) => respond(
            &mut stream,
            200,
            "OK",
            "",
            &Value::Array(answers).to_string(),
        ),
        (_, [one]) => respond(&mut stream, 200, "OK", "", &one.to_string()),
        _ => refuse(&mut stream, 400, "Bad Request", "Invalid request."),
    }
}

/// The response to a message, if it is one that has a response: a request, not a notification or
/// the client's answer to something.
fn answer(server: &Server<'_>, message: &Value) -> Option<Value> {
    message.get("method")?;
    server.handle(message)
}

pub(crate) fn serve(repositories: Repositories<'_>, host: &str, port: u16) -> Result<bool, String> {
    if !loopback(host) {
        return Err(
            "--host must be a loopback address (127.0.0.1, ::1 or localhost): the HTTP server does not sign callers in."
                .to_owned(),
        );
    }
    let listener = TcpListener::bind((host.trim_start_matches('[').trim_end_matches(']'), port))
        .map_err(|_| "The HTTP server could not listen on that address and port.".to_owned())?;
    let bound = listener
        .local_addr()
        .map_err(|_| "The HTTP server has no address.".to_owned())?;
    eprintln!("debuggatha: MCP listening on http://{bound}/mcp");
    let server = Server::new(repositories, None);
    let active = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for stream in listener.incoming().flatten() {
            if active.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                active.fetch_sub(1, Ordering::SeqCst);
                let mut stream = stream;
                refuse(
                    &mut stream,
                    503,
                    "Service Unavailable",
                    "Too many connections.",
                );
                continue;
            }
            let (server, active) = (&server, &active);
            scope.spawn(move || {
                connection(stream, server);
                active.fetch_sub(1, Ordering::SeqCst);
            });
        }
    });
    Ok(false)
}
