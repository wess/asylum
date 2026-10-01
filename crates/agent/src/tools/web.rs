use super::{def, need, Ctx, Outcome};
use crate::part::Part;
use anyhow::Result;
use grok::ToolDef;
use serde_json::{json, Value};

pub fn defs() -> Vec<ToolDef> {
  vec![
    def("web_search", "Search the web. Returns titles, links, and snippets.", json!({"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]})),
    def(
      "fetch_url",
      "Fetch a web page or file over HTTP as readable text (no JavaScript, no sign-in). Use the browser for interactive or signed-in pages.",
      json!({"type": "object", "properties": {"url": {"type": "string"}}, "required": ["url"]}),
    ),
  ]
}

pub async fn call(ctx: &Ctx<'_>, name: &str, args: &Value) -> Option<Result<Outcome>> {
  let r = match name {
    "web_search" => match need(args, "query") {
      Ok(q) => computer::web::search(q, 8, ctx.rt.web_proxy()).await.map(|hits| {
        if hits.is_empty() {
          return Outcome::text("No results.");
        }
        let text = hits
          .iter()
          .enumerate()
          .map(|(i, h)| format!("{}. {}\n   {}\n   {}", i + 1, h.title, h.url, h.snippet))
          .collect::<Vec<_>>()
          .join("\n");
        Outcome::text(format!("[untrusted web content]\n{text}"))
      }),
      Err(e) => Err(e),
    },
    "fetch_url" => match need(args, "url") {
      Ok(u) => computer::web::fetch(u, ctx.rt.web_proxy()).await.map(|p| Outcome {
        text: format!("[untrusted web content]\nURL: {}\nTitle: {}\n\n{}", p.url, p.title, p.text),
        parts: vec![Part::Link { url: p.url.clone(), title: p.title.clone() }],
        stop: false,
      }),
      Err(e) => Err(e),
    },
    _ => return None,
  };
  Some(r)
}
