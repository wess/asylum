//! `asylum call` - make a masked outbound API request through the secrets proxy.
//!
//! The agent references a configured upstream by name; Asylum injects the real
//! credential server-side. This command just forwards to the local proxy at
//! `$ASYLUM_PROXY_URL` with the proxy token from `$ASYLUM_PROXY_TOKEN` - it
//! never handles the upstream's secret. See `proxy::SKILL`.
//!
//! ```text
//! asylum call                                   # list available upstreams
//! asylum call <upstream> <METHOD> <path> [--data <body>|--data @file]
//! ```

use std::io::Write;
use std::process::Command;

pub fn call(args: &[String]) -> Result<(), String> {
    if crate::has_flag(args, "--skill") {
        println!("{}", proxy::SKILL);
        return Ok(());
    }
    let base = std::env::var(proxy::ENV_URL).map_err(|_| {
        format!(
            "not inside an Asylum run, or the secrets proxy is disabled ({} unset)",
            proxy::ENV_URL
        )
    })?;
    let base = base.trim_end_matches('/').to_string();
    let token = std::env::var(proxy::ENV_TOKEN).unwrap_or_default();
    let auth = format!("Authorization: Bearer {token}");

    let positional = crate::positionals(args);

    // No args: list the upstreams this run may address.
    let Some(upstream) = positional.first() else {
        return run_curl(&["-sS", "-H", &auth, &format!("{base}/")]);
    };

    let method = positional
        .get(1)
        .map(|s| s.to_uppercase())
        .unwrap_or_else(|| "GET".into());
    let path = positional.get(2).map(String::as_str).unwrap_or("/");
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    let target = format!("{base}/{upstream}{path}");

    let mut curl: Vec<String> = vec!["-sS".into(), "-X".into(), method, target, "-H".into(), auth];
    // `--data <body>` or `--data @file` (curl reads @file natively).
    if let Some(data) = crate::flag(args, "--data").or_else(|| crate::flag(args, "-d")) {
        curl.push("-H".into());
        curl.push("Content-Type: application/json".into());
        curl.push("--data-binary".into());
        curl.push(data.to_string());
    }
    run_curl(&curl.iter().map(String::as_str).collect::<Vec<_>>())
}

/// Run `curl <args>`, streaming its stdout to ours (the response body) and its
/// stderr to ours; error out on a transport failure.
fn run_curl(args: &[&str]) -> Result<(), String> {
    let out = Command::new("curl")
        .args(args)
        .output()
        .map_err(|e| format!("could not run curl: {e}"))?;
    std::io::stdout().write_all(&out.stdout).ok();
    if !out.stdout.is_empty() && !out.stdout.ends_with(b"\n") {
        println!();
    }
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(())
}
