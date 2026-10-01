//! Where Bots' models come from. A `Profile` (in `config`) names an HTTP
//! chat-completions endpoint or a coding-agent CLI; `preset` builds the
//! common ones, `credential` resolves a profile's key from wherever it
//! lives, and `Provider` streams a completion from either kind.

pub mod credential;
pub mod known;
pub mod preset;
pub mod process;
pub mod streamjson;

use anyhow::{bail, Result};
use config::{Kind, Profile};
use futures::stream::{BoxStream, StreamExt};
use grok::{Message, Request, StreamEvent, ToolDef};

#[derive(Clone)]
pub enum Provider {
  Http { client: grok::Client, model: String },
  Process { run: process::Run, model: String },
}

impl Provider {
  pub async fn open(profile: &Profile, model: &str, workspace: &std::path::Path, retries: u32) -> Result<Self> {
    match profile.kind {
      Kind::Http => {
        if profile.endpoint.trim().is_empty() {
          bail!("{} has no endpoint", profile.name);
        }
        let key = credential::resolve(&profile.credential).await?.unwrap_or_default();
        let client = grok::Client::new(key, Some(profile.endpoint.clone())).retries(retries as usize);
        Ok(Provider::Http { client, model: model.to_string() })
      }
      Kind::Process => {
        if profile.command.trim().is_empty() {
          bail!("{} has no command", profile.name);
        }
        Ok(Provider::Process {
          run: process::Run {
            command: profile.command.clone(),
            args: profile.args.clone(),
            output: profile.output,
            workspace: workspace.to_path_buf(),
          },
          model: model.to_string(),
        })
      }
    }
  }

  pub fn model(&self) -> &str {
    match self {
      Provider::Http { model, .. } | Provider::Process { model, .. } => model,
    }
  }

  /// Whether the model can call our tools. CLI agents bring their own.
  pub fn tools(&self) -> bool {
    matches!(self, Provider::Http { .. })
  }

  /// Stream a completion. Tools are offered only to HTTP providers; when
  /// there are none the request omits them (some servers reject empty).
  pub async fn stream(&self, messages: Vec<Message>, tools: Vec<ToolDef>) -> Result<BoxStream<'static, Result<StreamEvent>>> {
    match self {
      Provider::Http { client, model } => {
        let mut req = Request::new(model, messages);
        req.tools = tools;
        Ok(client.stream(req).await?.boxed())
      }
      Provider::Process { run, model } => run.stream(model, &messages).await,
    }
  }

  /// One whole answer, for background work.
  pub async fn complete(&self, messages: Vec<Message>, max_tokens: Option<u32>) -> Result<String> {
    match self {
      Provider::Http { client, model } => {
        let mut req = Request::new(model, messages);
        req.temperature = Some(0.0);
        req.max_tokens = max_tokens;
        client.complete(req).await
      }
      Provider::Process { .. } => {
        let mut s = self.stream(messages, Vec::new()).await?;
        let mut text = String::new();
        while let Some(ev) = s.next().await {
          if let StreamEvent::Text(t) = ev? {
            text.push_str(&t);
          }
        }
        Ok(text)
      }
    }
  }
}

/// Models a profile offers: `/models` for HTTP, known aliases for CLIs.
pub async fn discover(profile: &Profile) -> Result<Vec<String>> {
  match profile.kind {
    Kind::Http => {
      let key = credential::resolve(&profile.credential).await?.unwrap_or_default();
      let client = grok::Client::new(key, Some(profile.endpoint.clone())).retries(1);
      let mut ids: Vec<String> = client.models().await?.into_iter().map(|m| m.id).collect();
      ids.sort();
      Ok(ids)
    }
    Kind::Process => Ok(known::for_preset(&profile.preset)),
  }
}
