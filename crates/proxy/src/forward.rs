//! Execute a planned forward against the real upstream via `curl`.
//!
//! The secret-bearing header is written to curl's stdin (a `--config` line), so
//! it never appears in the process's argv (and thus not in `ps` /
//! `/proc/<pid>/cmdline`). The request body goes through a temp file
//! (`--data-binary @file`) so it is off argv too and binary-safe. The upstream
//! status and body come back through separate output files. Those files hold
//! request and response payloads in the clear, so they live in a private
//! per-request directory (see [`Spool`]).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::plan::Plan;

/// A private scratch directory for one request's spool files, removed on drop.
///
/// `std::env::temp_dir()` is `/tmp` on Linux - world-writable and shared. A
/// predictable name there would let any local user read the request and
/// response bodies moving through the proxy, or pre-plant a symlink at the path
/// curl is about to write through. So the directory is created 0700 under a
/// random name, and `create_new` refuses to adopt anything already sitting
/// there. Cleanup rides on `Drop` so it still happens when a forward fails.
struct Spool {
    dir: PathBuf,
}

impl Spool {
    fn new() -> std::io::Result<Self> {
        let mut raw = [0u8; 12];
        getrandom::fill(&mut raw)
            .map_err(|e| std::io::Error::other(format!("random source unavailable: {e}")))?;
        let name: String = raw.iter().map(|b| format!("{b:02x}")).collect();
        let dir = std::env::temp_dir().join(format!("asylum-proxy-{name}"));
        create_private_dir(&dir)?;
        Ok(Self { dir })
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
}

impl Drop for Spool {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Create a directory only this user can enter, failing if the name is taken.
#[cfg(unix)]
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(dir)
}

#[cfg(not(unix))]
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::DirBuilder::new().create(dir)
}

/// The upstream's response.
pub struct Fetched {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

/// Forward `plan` with `method`/`content_type`/`body` and return the upstream's
/// response. The injected secret only ever travels to `plan.url`'s host.
pub fn forward(
    plan: &Plan,
    method: &str,
    content_type: Option<&str>,
    body: &[u8],
) -> std::io::Result<Fetched> {
    let spool = Spool::new()?;
    let body_path = spool.path("req.body");
    let hdr_path = spool.path("res.hdr");
    let out_path = spool.path("res.out");
    std::fs::write(&body_path, body)?;

    let mut args: Vec<String> = vec![
        "-sS".into(),
        "--connect-timeout".into(),
        "15".into(),
        "--max-time".into(),
        "300".into(),
        "-X".into(),
        method.to_ascii_uppercase(),
        plan.url.clone(),
        "--data-binary".into(),
        format!("@{}", body_path.display()),
        "-D".into(),
        hdr_path.display().to_string(),
        "-o".into(),
        out_path.display().to_string(),
        "-w".into(),
        "%{http_code}".into(),
        // Read the injected auth header from stdin, keeping the secret off argv.
        "--config".into(),
        "-".into(),
    ];
    if let Some(ct) = content_type {
        args.push("-H".into());
        args.push(format!("Content-Type: {ct}"));
    }

    let mut child = Command::new("curl")
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        let line = format!(
            "header = \"{}: {}\"\n",
            curl_config_escape(&plan.header_name),
            curl_config_escape(&plan.header_value)
        );
        let _ = stdin.write_all(line.as_bytes());
    }
    let out = child.wait_with_output()?;

    // curl writes `000` and exits non-zero when the request never completed at
    // all - DNS failure, TLS refusal, connect timeout. Report that as a
    // transport error so the caller answers 502; parsing it as a status would
    // put `HTTP/1.1 0 OK` on the wire. curl's own diagnostic names the cause,
    // and cannot carry the injected header (it is passed via `--config` stdin
    // and never echoed without `-v`).
    let status: u16 = String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .unwrap_or(0);
    if !out.status.success() || status == 0 {
        let why = String::from_utf8_lossy(&out.stderr);
        let why = why.trim();
        return Err(std::io::Error::other(if why.is_empty() {
            "curl exited without completing the request".to_string()
        } else {
            why.to_string()
        }));
    }

    Ok(Fetched {
        status,
        content_type: read_content_type(&hdr_path),
        body: std::fs::read(&out_path).unwrap_or_default(),
    })
}

/// Escape a value for a curl `--config` double-quoted string.
///
/// Newlines must go too, not just quotes and backslashes: a raw one would end
/// the `header` line and let whatever follows be read as further curl
/// directives. Config-controlled today, but this is the only thing standing
/// between a secret's contents and curl's option parser.
fn curl_config_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

/// Pull the `Content-Type` from a curl `-D` response-header dump.
fn read_content_type(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .find(|l| l.to_ascii_lowercase().starts_with("content-type:"))
        .and_then(|l| l.split_once(':'))
        .map(|(_, v)| v.trim().to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string())
}

#[cfg(test)]
#[path = "../tests/forward.rs"]
mod tests;
