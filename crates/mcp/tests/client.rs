use super::*;

#[test]
fn flattens_content() {
  let res = json!({"content": [{"type": "text", "text": "a"}, {"type": "image"}, {"type": "text", "text": "b"}]});
  assert_eq!(flatten(&res), "a\n[image]\nb");
  assert_eq!(flatten(&json!({"structuredContent": {"x": 1}})), "{\"x\":1}");
}

#[test]
fn tool_defaults() {
  let t: Tool = serde_json::from_value(json!({"name": "search", "annotations": {"readOnlyHint": true}})).unwrap();
  assert!(t.read_only());
  assert_eq!(t.input_schema["type"], "object");
}

// A real stdio round trip against a tiny scripted server.
#[tokio::test]
async fn stdio_round_trip() {
  let script = r#"while IFS= read -r line; do
    id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
    case "$line" in
      *initialize\"*) printf '{"jsonrpc":"2.0","id":%s,"result":{"serverInfo":{"name":"t"}}}\n' "$id" ;;
      *tools/list*) printf '{"jsonrpc":"2.0","method":"notifications/x"}\n{"jsonrpc":"2.0","id":%s,"result":{"tools":[{"name":"echo"}]}}\n' "$id" ;;
      *tools/call*) printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"hi"}]}}\n' "$id" ;;
    esac
  done"#;
  let s = crate::stdio::Stdio::spawn("sh", &["-c".into(), script.into()], &Default::default(), None).unwrap();
  let c = Client::stdio(s);
  c.initialize().await.unwrap();
  let tools = c.tools().await.unwrap();
  assert_eq!(tools[0].name, "echo");
  assert_eq!(c.call_tool("echo", json!({})).await.unwrap(), ("hi".to_string(), false));
}

// Live: a real remote MCP server over Streamable HTTP.
// `cargo test -p mcp -- --ignored deepwiki`
#[tokio::test]
#[ignore]
async fn deepwiki_live() {
  let h = crate::http::Http::new("https://mcp.deepwiki.com/mcp", Default::default(), None);
  let c = Client::http(h);
  c.initialize().await.unwrap();
  let tools = c.tools().await.unwrap();
  assert!(tools.iter().any(|t| t.name.contains("wiki") || t.name.contains("question")), "{tools:?}");
}
