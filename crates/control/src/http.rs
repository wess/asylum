//! A minimal blocking HTTP/1.1 reader and writer - just enough for the control
//! API. One request, one response, close. Mirrors the companion server's
//! transport (the control surface keeps its own copy so the two crates stay
//! decoupled) and is hardened the same way: read/write deadlines, header and
//! body size caps, and strict `Content-Length` parsing.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::router::Response;

/// Largest request head (request line + headers) we will buffer.
const MAX_HEADER_BYTES: usize = 64 * 1024;
/// Largest request body we will accept.
const MAX_BODY_BYTES: usize = 1024 * 1024;
/// Deadline on any single read or write, so a slow client cannot hold a worker.
const IO_TIMEOUT: Duration = Duration::from_secs(15);

/// Why reading a request stopped early.
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

/// Read one request from `stream`, dispatch it through `handler`, write the
/// response. `handler` receives `(method, path, body, authorization)`.
pub(crate) fn handle_connection(
    stream: &mut TcpStream,
    handler: impl Fn(&str, &str, &str, Option<&str>) -> Response,
) {
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    match read_request(stream) {
        Ok((method, path, body, auth)) => {
            let response = handler(&method, &path, &body, auth.as_deref());
            let _ = write_response(stream, &response);
        }
        Err(err) => {
            if let Some(response) = err.response() {
                let _ = write_response(stream, &response);
            }
        }
    }
}

fn read_request(
    stream: &mut TcpStream,
) -> Result<(String, String, String, Option<String>), ReadError> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
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
    let mut auth = None;
    for line in lines {
        let lower = line.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            let parsed = parse_content_length(v.trim()).ok_or(ReadError::BadRequest)?;
            if content_length.is_some_and(|prev| prev != parsed) {
                return Err(ReadError::BadRequest);
            }
            content_length = Some(parsed);
        } else if lower.starts_with("authorization:") {
            auth = line
                .split_once(':')
                .map(|(_, value)| value.trim().to_string());
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

    Ok((
        method,
        path,
        String::from_utf8_lossy(&body).into_owned(),
        auth,
    ))
}

/// One bounded read, mapping a timeout to [`ReadError::Timeout`].
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

/// Parse a `Content-Length` value, accepting a repeated same-value list but
/// rejecting anything non-numeric or internally inconsistent.
fn parse_content_length(value: &str) -> Option<usize> {
    let mut seen: Option<usize> = None;
    for part in value.split(',') {
        let n: usize = part.trim().parse().ok()?;
        if seen.is_some_and(|prev| prev != n) {
            return None;
        }
        seen = Some(n);
    }
    seen
}

fn write_response(stream: &mut TcpStream, response: &Response) -> std::io::Result<()> {
    let status_text = match response.status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let head = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Connection: close\r\n\r\n",
        response.status,
        status_text,
        response.content_type,
        response.body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(response.body.as_bytes())?;
    stream.flush()
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// A fixed-window rate limiter for state-changing control requests (spawn,
/// check, activity), shared across worker threads, so a runaway agent cannot
/// flood the app with queued work.
pub(crate) struct RateLimiter {
    window: Duration,
    max: u32,
    state: Mutex<(Instant, u32)>,
}

impl RateLimiter {
    pub(crate) fn new(window: Duration, max: u32) -> Self {
        Self {
            window,
            max,
            state: Mutex::new((Instant::now(), 0)),
        }
    }

    pub(crate) fn allow(&self) -> bool {
        self.allow_at(Instant::now())
    }

    pub(crate) fn allow_at(&self, now: Instant) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let (start, count) = &mut *state;
        if now.duration_since(*start) > self.window {
            *start = now;
            *count = 0;
        }
        *count += 1;
        *count <= self.max
    }
}
