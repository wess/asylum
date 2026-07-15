use super::*;

#[test]
fn parses_host_and_port() {
    let c = Client::new("http://127.0.0.1:8788", "").unwrap();
    // No token, no panic; the struct fields are private, so we exercise via new.
    let _ = c;
    assert!(Client::new("http://localhost:9000/", "tok").is_ok());
}

#[test]
fn rejects_non_http_and_portless_urls() {
    assert!(Client::new("https://127.0.0.1:8788", "").is_err());
    assert!(Client::new("127.0.0.1:8788", "").is_err());
    assert!(Client::new("http://127.0.0.1", "").is_err());
}
