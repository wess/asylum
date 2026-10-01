use super::*;

#[test]
fn parses_targets() {
  assert_eq!(target("CONNECT api.github.com:443 HTTP/1.1"), Some(("api.github.com".into(), 443, true)));
  assert_eq!(target("GET http://Example.com/a/b HTTP/1.1"), Some(("example.com".into(), 80, false)));
  assert_eq!(target("GET http://example.com:8080/ HTTP/1.1"), Some(("example.com".into(), 8080, false)));
  assert_eq!(target("GET /relative HTTP/1.1"), None);
}

async fn echo_server() -> u16 {
  let l = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
  let port = l.local_addr().unwrap().port();
  tokio::spawn(async move {
    while let Ok((mut s, _)) = l.accept().await {
      tokio::spawn(async move {
        let mut b = [0u8; 4096];
        let n = s.read(&mut b).await.unwrap_or(0);
        let got = String::from_utf8_lossy(&b[..n]).to_string();
        let body = got.lines().next().unwrap_or_default().to_string();
        let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes()).await;
      });
    }
  });
  port
}

async fn ask(proxy: u16, req: String) -> String {
  let mut s = TcpStream::connect(("127.0.0.1", proxy)).await.unwrap();
  s.write_all(req.as_bytes()).await.unwrap();
  let mut out = Vec::new();
  let _ = tokio::time::timeout(std::time::Duration::from_secs(2), s.read_to_end(&mut out)).await;
  String::from_utf8_lossy(&out).to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn allows_and_blocks() {
  let up = echo_server().await;
  let proxy = start(Arc::new(|h: &str| h == "127.0.0.1")).await.unwrap();
  // Plain HTTP to an allowed host is forwarded in origin form.
  let r = ask(proxy, format!("GET http://127.0.0.1:{up}/hello HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")).await;
  assert!(r.contains("200 OK") && r.contains("GET /hello HTTP/1.1"), "{r}");
  // CONNECT to an allowed host tunnels.
  let mut s = TcpStream::connect(("127.0.0.1", proxy)).await.unwrap();
  s.write_all(format!("CONNECT 127.0.0.1:{up} HTTP/1.1\r\n\r\n").as_bytes()).await.unwrap();
  let mut b = [0u8; 256];
  let n = s.read(&mut b).await.unwrap();
  assert!(String::from_utf8_lossy(&b[..n]).contains("200 Connection Established"));
  s.write_all(b"PING /x HTTP/1.1\r\n\r\n").await.unwrap();
  let n = s.read(&mut b).await.unwrap();
  assert!(String::from_utf8_lossy(&b[..n]).contains("PING /x"));
  // Anything else is refused.
  let r = ask(proxy, "CONNECT evil.example:443 HTTP/1.1\r\n\r\n".into()).await;
  assert!(r.contains("403") && r.contains(BLOCKED));
  let r = ask(proxy, "GET http://localhost.evil/ HTTP/1.1\r\n\r\n".into()).await;
  assert!(r.contains("403"));
}
