use super::*;

#[test]
fn pkce_shapes() {
  let (v, c) = pkce();
  assert!(v.len() >= 43);
  assert_eq!(c.len(), 43);
  assert_ne!(v, c);
}

#[test]
fn query_parsing() {
  let q = parse_query("/callback?code=a%2Fb&state=s");
  assert_eq!(q["code"], "a/b");
  assert_eq!(q["state"], "s");
}

#[test]
fn origin_of_url() {
  assert_eq!(origin("https://mcp.linear.app/sse"), "https://mcp.linear.app");
}

#[tokio::test]
async fn loopback_receives_code() {
  let (listener, redirect) = loopback().await.unwrap();
  let url = format!("{redirect}?code=abc&state=xyz");
  let hit = tokio::spawn(async move { reqwest::get(url).await.unwrap().status() });
  let code = wait_code(&listener, "xyz", std::time::Duration::from_secs(5)).await.unwrap();
  assert_eq!(code, "abc");
  assert!(hit.await.unwrap().is_success());
}

#[test]
fn expiry() {
  let t = Tokens { expires_at: Some(100_000), ..Default::default() };
  assert!(t.expired(50_000));
  assert!(!t.expired(10_000));
}
