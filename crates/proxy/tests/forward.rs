use super::*;

/// The escape stands between a secret's bytes and curl's option parser: a raw
/// newline would end the `header` line and let the rest be read as directives.
#[test]
fn config_escape_neutralizes_line_breaks_and_quotes() {
    assert_eq!(curl_config_escape("plain"), "plain");
    assert_eq!(curl_config_escape(r#"say "hi""#), r#"say \"hi\""#);
    assert_eq!(curl_config_escape(r"back\slash"), r"back\\slash");

    // The important one: no escaped value may contain a real line break.
    for raw in [
        "sk-abc\nuser-agent = \"pwn\"",
        "sk-abc\r\nproxy = \"http://evil\"",
        "sk-abc\rinsecure",
    ] {
        let escaped = curl_config_escape(raw);
        assert!(
            !escaped.contains('\n') && !escaped.contains('\r'),
            "line break survived escaping: {escaped:?}"
        );
    }

    // Backslashes are doubled before quotes are escaped, so a trailing
    // backslash cannot escape the closing quote.
    let escaped = curl_config_escape(r"trailing\");
    assert_eq!(escaped, r"trailing\\");
    assert!(format!("\"{escaped}\"").ends_with(r#"\\""#));
}

/// The spool holds request and response bodies in the clear, so on a shared
/// `/tmp` it must not be readable by other users or guessable in advance.
#[test]
fn spool_is_private_random_and_self_cleaning() {
    let a = Spool::new().unwrap();
    let b = Spool::new().unwrap();
    assert_ne!(a.dir, b.dir, "spool names must not be predictable");
    assert!(a.dir.is_dir());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&a.dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700, "spool must be owner-only");
    }

    // Paths land inside the private directory, not next to it.
    let p = a.path("req.body");
    assert_eq!(p.parent().unwrap(), a.dir);

    let dir = a.dir.clone();
    std::fs::write(a.path("req.body"), b"secret body").unwrap();
    drop(a);
    assert!(
        !dir.exists(),
        "spool must be removed on drop, even with files in it"
    );

    let dir_b = b.dir.clone();
    drop(b);
    assert!(!dir_b.exists());
}

/// A missing or malformed header dump must not panic or invent a type.
#[test]
fn content_type_falls_back_when_absent() {
    let dir = Spool::new().unwrap();
    let hdr = dir.path("res.hdr");

    std::fs::write(
        &hdr,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n",
    )
    .unwrap();
    assert_eq!(read_content_type(&hdr), "application/json");

    // Header casing varies by server.
    std::fs::write(&hdr, "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\n").unwrap();
    assert_eq!(read_content_type(&hdr), "text/plain");

    std::fs::write(&hdr, "HTTP/1.1 204 No Content\r\n").unwrap();
    assert_eq!(read_content_type(&hdr), "application/octet-stream");

    assert_eq!(
        read_content_type(&dir.path("nope.hdr")),
        "application/octet-stream"
    );
}
