//! A minimal blocking HTTP/1.1 request reader and response writer - just enough
//! for the companion API. One request, one response, close. Hardened against
//! slow and oversized clients: read/write deadlines, a header and body size cap,
//! and strict `Content-Length` parsing, so a single misbehaving connection
//! cannot stall or exhaust the server.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::router::Response;

/// The custom header our own page sends on state-changing requests. A browser
/// cannot set it on a cross-site "simple request" without a CORS preflight we
/// never approve, so requiring it on mutations blocks silent cross-site POSTs
/// (CSRF) while same-origin fetches from `/app.js` pass.
pub(crate) const CSRF_HEADER: &str = "x-asylum-companion";

/// Largest request head (request line + headers) we will buffer.
const MAX_HEADER_BYTES: usize = 64 * 1024;
/// Largest request body we will accept.
const MAX_BODY_BYTES: usize = 1024 * 1024;
/// Deadline on any single read or write, so a slow client cannot hold a worker.
const IO_TIMEOUT: Duration = Duration::from_secs(15);

/// Why reading a request stopped early.
enum ReadError {
    /// A read or write stalled past [`IO_TIMEOUT`].
    Timeout,
    /// The head or body exceeded its size cap.
    TooLarge,
    /// Malformed request line or headers (incl. a bad/conflicting length).
    BadRequest,
    /// The peer closed before sending a complete request; nothing to answer.
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

/// Read one request from `stream`, dispatch it through `handler`, and write the
/// response. `handler` receives `(method, path, body, authorization, csrf)`,
/// where `csrf` is the value of the [`CSRF_HEADER`] if present.
pub(crate) fn handle_connection(
    stream: &mut TcpStream,
    handler: impl Fn(&str, &str, &str, Option<&str>, Option<&str>) -> Response,
) {
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    match read_request(stream) {
        Ok(req) => {
            let response = handler(
                &req.method,
                &req.path,
                &req.body,
                req.auth.as_deref(),
                req.csrf.as_deref(),
            );
            let _ = write_response(stream, &response);
        }
        Err(err) => {
            if let Some(response) = err.response() {
                let _ = write_response(stream, &response);
            }
        }
    }
}

/// A parsed request: enough of it for the companion API.
struct Request {
    method: String,
    path: String,
    body: String,
    auth: Option<String>,
    csrf: Option<String>,
}

/// Read the request line, headers, and (Content-Length) body, plus the
/// `Authorization` and CSRF header values if present.
fn read_request(stream: &mut TcpStream) -> Result<Request, ReadError> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    // Read until we have the header terminator, bounded by size and deadline.
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
    let mut csrf = None;
    for line in lines {
        let lower = line.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            // Reject a missing/garbled or conflicting length outright.
            let parsed = parse_content_length(v.trim()).ok_or(ReadError::BadRequest)?;
            if content_length.is_some_and(|prev| prev != parsed) {
                return Err(ReadError::BadRequest);
            }
            content_length = Some(parsed);
        } else if lower.starts_with("authorization:") {
            auth = line
                .split_once(':')
                .map(|(_, value)| value.trim().to_string());
        } else if lower.starts_with(CSRF_HEADER) {
            csrf = line
                .split_once(':')
                .map(|(_, value)| value.trim().to_string());
        }
    }

    let content_length = content_length.unwrap_or(0);
    if content_length > MAX_BODY_BYTES {
        return Err(ReadError::TooLarge);
    }

    // Body bytes already read past the header terminator.
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
        body: String::from_utf8_lossy(&body).into_owned(),
        auth,
        csrf,
    })
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

/// Parse a `Content-Length` value, accepting a repeated same-value list
/// (`5, 5`) but rejecting anything non-numeric or internally inconsistent.
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
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "OK",
    };
    // No `Access-Control-Allow-Origin`: the page is same-origin, so an arbitrary
    // website must not be able to read these responses. A restrictive CSP plus
    // `nosniff` are defense in depth for the served HTML.
    let head = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Content-Security-Policy: {}\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Referrer-Policy: no-referrer\r\n\
         Connection: close\r\n\r\n",
        response.status,
        status_text,
        response.content_type,
        response.body.len(),
        crate::router::CSP,
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(response.body.as_bytes())?;
    stream.flush()
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// A fixed-window rate limiter for state-changing requests, shared across
/// worker threads. Bounds how fast a client can drive mutating endpoints
/// (follow-ups) so a flood cannot starve normal traffic or spawn work without
/// limit.
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

    /// Whether a mutation is allowed right now. Resets the count each window.
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
