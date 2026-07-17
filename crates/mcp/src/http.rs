//! Minimal blocking HTTP/1.1 transport for the gateway's *server* side - the
//! agent POSTs one JSON-RPC message and reads one JSON response, connection
//! closed. Same hardening as the proxy and control servers: read/write
//! deadlines, header and body caps, and strict `Content-Length` parsing.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// A parsed inbound request.
pub(crate) struct Request {
    pub method: String,
    pub path: String,
    pub bearer: Option<String>,
    pub body: String,
}

/// A response to write back.
pub(crate) struct Response {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl Response {
    pub(crate) fn text(status: u16, msg: &str) -> Self {
        Response {
            status,
            content_type: "text/plain; charset=utf-8".into(),
            body: msg.as_bytes().to_vec(),
        }
    }
    pub(crate) fn json(status: u16, body: String) -> Self {
        Response {
            status,
            content_type: "application/json".into(),
            body: body.into_bytes(),
        }
    }
}

const MAX_HEADER_BYTES: usize = 64 * 1024;
/// MCP tool results can be large (file contents, search hits); cap generously.
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(30);

enum ReadError {
    Timeout,
    TooLarge,
    BadRequest,
    Closed,
}

impl ReadError {
    fn response(&self) -> Option<Response> {
        match self {
            ReadError::Timeout => Some(Response::text(408, "request timeout")),
            ReadError::TooLarge => Some(Response::text(413, "payload too large")),
            ReadError::BadRequest => Some(Response::text(400, "bad request")),
            ReadError::Closed => None,
        }
    }
}

pub(crate) fn handle_connection(stream: &mut TcpStream, handler: impl Fn(Request) -> Response) {
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    match read_request(stream) {
        Ok(req) => {
            let response = handler(req);
            let _ = write_response(stream, &response);
        }
        Err(err) => {
            if let Some(response) = err.response() {
                let _ = write_response(stream, &response);
            }
        }
    }
}

fn read_request(stream: &mut TcpStream) -> Result<Request, ReadError> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 2048];
    let header_end = loop {
        let n = read_chunk(stream, &mut chunk)?;
        if n == 0 {
            return Err(ReadError::Closed);
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = find_subsequence(&buf, b"\r\n\r\n") {
            break pos;
        }
        if buf.len() > MAX_HEADER_BYTES {
            return Err(ReadError::TooLarge);
        }
    };

    let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let mut lines = head.lines();
    let request_line = lines.next().ok_or(ReadError::BadRequest)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or(ReadError::BadRequest)?.to_string();
    let path = parts.next().ok_or(ReadError::BadRequest)?.to_string();

    let mut content_length: Option<usize> = None;
    let mut bearer = None;
    for line in lines {
        let lower = line.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            let parsed = parse_content_length(v.trim()).ok_or(ReadError::BadRequest)?;
            if content_length.is_some_and(|prev| prev != parsed) {
                return Err(ReadError::BadRequest);
            }
            content_length = Some(parsed);
        } else if lower.starts_with("authorization:") {
            bearer = line.split_once(':').and_then(|(_, v)| {
                v.trim()
                    .strip_prefix("Bearer ")
                    .map(|t| t.trim().to_string())
            });
        }
    }

    let content_length = content_length.unwrap_or(0);
    if content_length > MAX_BODY_BYTES {
        return Err(ReadError::TooLarge);
    }

    let body_start = header_end + 4;
    let mut body = buf[body_start.min(buf.len())..].to_vec();
    while body.len() < content_length {
        let n = read_chunk(stream, &mut chunk)?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
        if body.len() > MAX_BODY_BYTES {
            return Err(ReadError::TooLarge);
        }
    }
    body.truncate(content_length);

    Ok(Request {
        method,
        path,
        bearer,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

fn read_chunk(stream: &mut TcpStream, chunk: &mut [u8]) -> Result<usize, ReadError> {
    match stream.read(chunk) {
        Ok(n) => Ok(n),
        Err(e) if is_timeout(&e) => Err(ReadError::Timeout),
        Err(_) => Err(ReadError::Closed),
    }
}

fn is_timeout(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    )
}

fn parse_content_length(value: &str) -> Option<usize> {
    let mut seen: Option<usize> = None;
    for part in value.split(',') {
        let part = part.trim();
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let n: usize = part.parse().ok()?;
        if seen.is_some_and(|prev| prev != n) {
            return None;
        }
        seen = Some(n);
    }
    seen
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "OK",
    }
}

fn write_response(stream: &mut TcpStream, response: &Response) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Connection: close\r\n\r\n",
        response.status,
        reason_phrase(response.status),
        response.content_type,
        response.body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
#[path = "../tests/http.rs"]
mod tests;
