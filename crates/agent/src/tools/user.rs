//! Talking to the user through cards: questions, forms, secure secrets,
//! drafts to approve, structured replies, and files.

use super::{arg, def, need, Ctx, Outcome};
use crate::part::{Field, Part};
use crate::runtime::Reply;
use anyhow::{bail, Result};
use grok::ToolDef;
use serde_json::{json, Value};

pub fn defs() -> Vec<ToolDef> {
  vec![
    def(
      "ask_user",
      "Ask the user a question and end your turn. Offer short options when there are clear choices.",
      json!({"type": "object", "properties": {"question": {"type": "string"}, "options": {"type": "array", "items": {"type": "string"}}}, "required": ["question"]}),
    ),
    def(
      "request_secret",
      "Ask the user to enter a secret (API key, token) securely. It is stored in the keychain and set as an env var named NAME; you never see the value. Give `fill` (an element number) to type it into the current page instead. Waits for the user.",
      json!({"type": "object", "properties": {"name": {"type": "string", "description": "like API_TOKEN"}, "description": {"type": "string"}, "fill": {"type": "integer"}}, "required": ["name", "description"]}),
    ),
    def(
      "show_form",
      "Ask the user to fill a short form (one per step), e.g. a shipping address or account details. Ends your turn; their answers arrive as a message.",
      json!({"type": "object", "properties": {
        "title": {"type": "string"},
        "fields": {"type": "array", "items": {"type": "object", "properties": {"name": {"type": "string"}, "label": {"type": "string"}, "kind": {"type": "string", "enum": ["text", "email", "phone", "number", "date", "long"]}, "required": {"type": "boolean"}}, "required": ["name", "label"]}}
      }, "required": ["title", "fields"]}),
    ),
    def(
      "draft_email",
      "Draft an email for the user to review. They can edit and send it or discard it.",
      json!({"type": "object", "properties": {"to": {"type": "array", "items": {"type": "string"}}, "subject": {"type": "string"}, "body": {"type": "string"}}, "required": ["to", "subject", "body"]}),
    ),
    def(
      "draft_slack",
      "Draft a Slack message for the user to review before sending.",
      json!({"type": "object", "properties": {"channel": {"type": "string"}, "body": {"type": "string"}}, "required": ["channel", "body"]}),
    ),
    def(
      "show_card",
      "Show structured results: kind \"table\" (data: {columns: [..], rows: [[..]]}), \"board\" (data: {columns: [{title, items: [..]}]}), \"chart\" (data: {type: bar|line|pie, labels: [..], series: [{name, values: [..]}]}), or \"card\" (data: {fields: {label: value}}).",
      json!({"type": "object", "properties": {"kind": {"type": "string", "enum": ["table", "board", "chart", "card"]}, "title": {"type": "string"}, "data": {"type": "object"}}, "required": ["kind", "title", "data"]}),
    ),
    def("share_file", "Show a workspace file to the user as a card they can preview and save.", json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]})),
  ]
}

pub async fn call(ctx: &Ctx<'_>, name: &str, args: &Value) -> Option<Result<Outcome>> {
  let r = match name {
    "ask_user" => {
      let options: Vec<String> = args["options"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect();
      Ok(Outcome {
        text: "Asked. End your turn now and wait for the reply.".into(),
        parts: vec![Part::Question { text: arg(args, "question").into(), options }],
        stop: true,
      })
    }
    "request_secret" => secret(ctx, args).await,
    "show_form" => {
      let fields: Vec<Field> = serde_json::from_value(args["fields"].clone()).unwrap_or_default();
      Ok(Outcome {
        text: "Form shown. End your turn; the answers will arrive as a message.".into(),
        parts: vec![Part::Form { title: arg(args, "title").into(), fields, status: "pending".into(), values: json!({}) }],
        stop: true,
      })
    }
    "draft_email" => Ok(Outcome::with(
      "Draft ready for the user to review and send.",
      Part::Draft {
        kind: "email".into(),
        to: args["to"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect(),
        subject: arg(args, "subject").into(),
        body: arg(args, "body").into(),
        status: "draft".into(),
        channel: String::new(),
      },
    )),
    "draft_slack" => Ok(Outcome::with(
      "Draft ready for the user to review and send.",
      Part::Draft { kind: "slack".into(), to: vec![], subject: String::new(), body: arg(args, "body").into(), status: "draft".into(), channel: arg(args, "channel").into() },
    )),
    "show_card" => Ok(Outcome::with(
      "Shown.",
      Part::Card { kind: arg(args, "kind").into(), title: arg(args, "title").into(), data: args["data"].clone() },
    )),
    "share_file" => need(args, "path").and_then(|p| {
      let ws = ctx.rt.computer.workspace();
      let full = computer::fs::resolve(&ws, p)?;
      if !full.exists() {
        bail!("{p} does not exist");
      }
      let shown = computer::fs::show(&ws, &full);
      Ok(Outcome::with("Shared.", super::computer::file_part(&ws, &shown)))
    }),
    _ => return None,
  };
  Some(r)
}

async fn secret(ctx: &Ctx<'_>, args: &Value) -> Result<Outcome> {
  let name = need(args, "name")?.to_uppercase();
  if !store::secrets::valid_name(&name) {
    bail!("secret names look like API_TOKEN");
  }
  let fill = args["fill"].as_u64().map(|n| n as u32);
  let desc = arg(args, "description").to_string();
  let i = ctx.sheet.push(Part::SecretRequest { name: name.clone(), description: desc.clone(), status: "pending".into(), fill });
  ctx.sheet.save(ctx.rt).await?;
  crate::turn::attention(ctx.rt, ctx.bot, ctx.chat, "Needs a secret", &desc).await;
  let rx = ctx.rt.wait(&format!("secret:{}:{name}", ctx.bot.id)).await;
  let (status, text) = tokio::select! {
    r = rx => match r {
      Ok(Reply::Secret { filled: Some(true) }) => ("filled", "Filled into the page. Secret values were never shown to you.".to_string()),
      Ok(Reply::Secret { filled: Some(false) }) => ("failed", "Could not fill into the page.".to_string()),
      Ok(Reply::Secret { filled: None }) => ("saved", format!("Saved securely. It is available as ${name} in your shell from your next command.")),
      _ => ("skipped", "The user did not provide it.".to_string()),
    },
    _ = ctx.cancel.cancelled() => ("skipped", "Stopped.".to_string()),
  };
  ctx.sheet.set(i, Part::SecretRequest { name, description: desc, status: status.into(), fill });
  crate::turn::resume(ctx.rt, ctx.bot, ctx.chat).await;
  Ok(Outcome::text(text))
}
