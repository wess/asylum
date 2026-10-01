//! Where Bots' models come from. A `Profile` (in `config`) names an HTTP
//! chat-completions endpoint or a coding-agent CLI; `preset` builds the
//! common ones, `credential` resolves a profile's key from wherever it
//! lives, and `Provider` streams a completion from either kind.

pub mod credential;
pub mod litellm;
pub mod known;
pub mod preset;
pub mod process;
pub mod streamjson;

use anyhow::{bail, Result};
use config::{Kind, Profile};
use futures::stream::{BoxStream, StreamExt};
use chat::{Message, Request, StreamEvent, ToolDef};

#[derive(Clone)]
pub enum Provider {
  Http { client: chat::Client, model: String, reasoning: Option<String> },
  Process { run: process::Run, model: String },
  /// Try each in order; move on only when one fails before saying anything,
  /// so a failed provider can never leave half an answer behind.
  Fallback(Vec<Provider>),
}

impl Provider {
  pub async fn open(profile: &Profile, model: &str, workspace: &std::path::Path, retries: u32) -> Result<Self> {
    match profile.kind {
      Kind::Http => {
        if profile.endpoint.trim().is_empty() {
          bail!("{} has no endpoint", profile.name);
        }
        let key = credential::resolve_for(profile).await?.unwrap_or_default();
        let client = chat::Client::new(key, Some(profile.endpoint.clone())).retries(retries as usize);
        let reasoning = Some(profile.reasoning.trim().to_string()).filter(|r| !r.is_empty());
        Ok(Provider::Http { client, model: model.to_string(), reasoning })
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
      Provider::Fallback(chain) => chain.first().map(|p| p.model()).unwrap_or_default(),
    }
  }

  /// Whether the model can call our tools. CLI agents bring their own.
  pub fn tools(&self) -> bool {
    match self {
      Provider::Http { .. } => true,
      Provider::Process { .. } => false,
      Provider::Fallback(chain) => chain.first().is_some_and(|p| p.tools()),
    }
  }

  /// Chain `fallbacks` behind this provider. Only providers that call tools
  /// the same way as this one can stand in for it.
  pub fn with_fallbacks(self, fallbacks: Vec<Provider>) -> Provider {
    let tools = self.tools();
    let mut chain = vec![self];
    chain.extend(fallbacks.into_iter().filter(|p| p.tools() == tools));
    if chain.len() == 1 {
      chain.pop().expect("one provider")
    } else {
      Provider::Fallback(chain)
    }
  }

  /// Stream a completion. Tools are offered only to HTTP providers; when
  /// there are none the request omits them (some servers reject empty).
  pub async fn stream(&self, messages: Vec<Message>, tools: Vec<ToolDef>) -> Result<BoxStream<'static, Result<StreamEvent>>> {
    match self {
      Provider::Http { client, model, reasoning } => {
        let mut req = Request::new(model, messages);
        req.tools = tools;
        req.reasoning_effort = reasoning.clone();
        Ok(client.stream(req).await?.boxed())
      }
      Provider::Process { run, model } => run.stream(model, &messages).await,
      Provider::Fallback(chain) => {
        let mut last = None;
        for p in chain {
          // Open the stream and wait for its first event: a failure before
          // anything arrives moves on; anything after that is this provider's.
          let opened = Box::pin(p.stream(messages.clone(), tools.clone())).await;
          match opened {
            Ok(mut s) => match s.next().await {
              Some(Ok(first)) => return Ok(futures::stream::once(async move { Ok(first) }).chain(s).boxed()),
              Some(Err(e)) => last = Some(e),
              None => return Ok(futures::stream::empty().boxed()),
            },
            Err(e) => last = Some(e),
          }
        }
        Err(last.unwrap_or_else(|| anyhow::anyhow!("no provider answered")))
      }
    }
  }

  /// One whole answer, for background work.
  pub async fn complete(&self, messages: Vec<Message>, max_tokens: Option<u32>) -> Result<String> {
    match self {
      Provider::Http { client, model, .. } => {
        let mut req = Request::new(model, messages);
        req.temperature = Some(0.0);
        req.max_tokens = max_tokens;
        client.complete(req).await
      }
      Provider::Fallback(chain) => {
        let mut last = None;
        for p in chain {
          match Box::pin(p.complete(messages.clone(), max_tokens)).await {
            Ok(t) => return Ok(t),
            Err(e) => last = Some(e),
          }
        }
        Err(last.unwrap_or_else(|| anyhow::anyhow!("no provider answered")))
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

/// A small real request to the model, to prove the key, the model name, and
/// upstream access all work (listing models alone can't show that). Billed.
pub async fn check(profile: &Profile, model: &str, workspace: &std::path::Path) -> Result<String> {
  let p = Provider::open(profile, model, workspace, 0).await?;
  let reply = p.complete(vec![Message::user("Reply with the single word: ready")], Some(8)).await?;
  Ok(reply.trim().chars().take(60).collect())
}

/// Models a profile offers: `/models` for HTTP, known aliases for CLIs.
pub async fn discover(profile: &Profile) -> Result<Vec<String>> {
  match profile.kind {
    Kind::Http => {
      let key = credential::resolve_for(profile).await?.unwrap_or_default();
      // A LiteLLM team scoped to a few models offers just those.
      if profile.team.is_some() {
        let teams = litellm::teams(&profile.endpoint, &key).await.unwrap_or_default();
        if let Some(t) = teams.iter().find(|t| Some(&t.id) == profile.team.as_ref()) {
          return litellm::models(&profile.endpoint, &key, Some(t)).await;
        }
      }
      let client = chat::Client::new(key, Some(profile.endpoint.clone())).retries(1);
      let mut ids: Vec<String> = client.models().await?.into_iter().map(|m| m.id).collect();
      ids.sort();
      Ok(ids)
    }
    Kind::Process => Ok(known::for_preset(&profile.preset)),
  }
}
