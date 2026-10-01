use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
  System,
  User,
  Assistant,
  Tool,
}

/// One piece of multimodal user content.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
  Text { text: String },
  ImageUrl { image_url: ImageUrl },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageUrl {
  pub url: String,
}

impl Part {
  pub fn text(text: impl Into<String>) -> Self {
    Part::Text { text: text.into() }
  }

  pub fn image(url: impl Into<String>) -> Self {
    Part::ImageUrl {
      image_url: ImageUrl { url: url.into() },
    }
  }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Content {
  Text(String),
  Parts(Vec<Part>),
}

impl Content {
  pub fn as_text(&self) -> String {
    match self {
      Content::Text(t) => t.clone(),
      Content::Parts(parts) => parts
        .iter()
        .filter_map(|p| match p {
          Part::Text { text } => Some(text.as_str()),
          Part::ImageUrl { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n"),
    }
  }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionCall {
  pub name: String,
  pub arguments: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
  pub id: String,
  #[serde(rename = "type", default = "function_kind")]
  pub kind: String,
  pub function: FunctionCall,
}

fn function_kind() -> String {
  "function".into()
}

impl ToolCall {
  pub fn new(id: impl Into<String>, name: impl Into<String>, arguments: impl Into<String>) -> Self {
    Self {
      id: id.into(),
      kind: function_kind(),
      function: FunctionCall {
        name: name.into(),
        arguments: arguments.into(),
      },
    }
  }

  pub fn args(&self) -> Value {
    serde_json::from_str(&self.function.arguments).unwrap_or(Value::Object(Default::default()))
  }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
  pub role: Role,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub content: Option<Content>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub tool_calls: Option<Vec<ToolCall>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub tool_call_id: Option<String>,
}

impl Message {
  fn base(role: Role, content: Option<Content>) -> Self {
    Self {
      role,
      content,
      name: None,
      tool_calls: None,
      tool_call_id: None,
    }
  }

  pub fn system(text: impl Into<String>) -> Self {
    Self::base(Role::System, Some(Content::Text(text.into())))
  }

  pub fn user(text: impl Into<String>) -> Self {
    Self::base(Role::User, Some(Content::Text(text.into())))
  }

  pub fn user_parts(parts: Vec<Part>) -> Self {
    Self::base(Role::User, Some(Content::Parts(parts)))
  }

  pub fn assistant(text: impl Into<String>, calls: Vec<ToolCall>) -> Self {
    let text = text.into();
    let mut m = Self::base(
      Role::Assistant,
      (!text.is_empty() || calls.is_empty()).then_some(Content::Text(text)),
    );
    m.tool_calls = (!calls.is_empty()).then_some(calls);
    m
  }

  pub fn tool(call_id: impl Into<String>, output: impl Into<String>) -> Self {
    let mut m = Self::base(Role::Tool, Some(Content::Text(output.into())));
    m.tool_call_id = Some(call_id.into());
    m
  }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Function {
  pub name: String,
  pub description: String,
  pub parameters: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolDef {
  #[serde(rename = "type")]
  pub kind: String,
  pub function: Function,
}

impl ToolDef {
  pub fn new(name: &str, description: &str, parameters: Value) -> Self {
    Self {
      kind: function_kind(),
      function: Function {
        name: name.into(),
        description: description.into(),
        parameters,
      },
    }
  }
}

#[derive(Clone, Debug, Serialize)]
pub struct Request {
  pub model: String,
  pub messages: Vec<Message>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub tools: Vec<ToolDef>,
  pub stream: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub temperature: Option<f32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub max_tokens: Option<u32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub stream_options: Option<Value>,
  /// "low", "medium", or "high" for reasoning models (OpenAI and LiteLLM).
  #[serde(skip_serializing_if = "Option::is_none")]
  pub reasoning_effort: Option<String>,
}

impl Request {
  pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
    Self {
      model: model.into(),
      messages,
      tools: Vec::new(),
      stream: true,
      temperature: None,
      max_tokens: None,
      stream_options: Some(serde_json::json!({ "include_usage": true })),
      reasoning_effort: None,
    }
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
  #[serde(default)]
  pub prompt_tokens: u64,
  #[serde(default)]
  pub completion_tokens: u64,
  #[serde(default)]
  pub total_tokens: u64,
}

/// A tool-call fragment as it arrives in a stream chunk.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct CallDelta {
  #[serde(default)]
  pub index: usize,
  pub id: Option<String>,
  pub function: Option<FunctionDelta>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct FunctionDelta {
  pub name: Option<String>,
  pub arguments: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct Delta {
  pub content: Option<String>,
  /// xAI and DeepSeek call it `reasoning_content`; Ollama calls it `reasoning`.
  #[serde(alias = "reasoning")]
  pub reasoning_content: Option<String>,
  pub tool_calls: Option<Vec<CallDelta>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Choice {
  #[serde(default)]
  pub delta: Delta,
  pub finish_reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Chunk {
  #[serde(default)]
  pub choices: Vec<Choice>,
  pub usage: Option<Usage>,
}

/// What the client yields while a completion streams.
#[derive(Clone, Debug, PartialEq)]
pub enum StreamEvent {
  Text(String),
  Reasoning(String),
  Usage(Usage),
  Done {
    calls: Vec<ToolCall>,
    finish: Option<String>,
  },
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Model {
  pub id: String,
  #[serde(default)]
  pub owned_by: String,
}

#[cfg(test)]
#[path = "../tests/types.rs"]
mod tests;
