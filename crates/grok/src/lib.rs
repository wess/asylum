//! xAI Grok API client. `types` is the wire model, `sse` and `accumulate` are
//! pure stream decoding, and `client` does the HTTP.

pub mod accumulate;
pub mod client;
pub mod sse;
pub mod types;

pub use accumulate::Accumulator;
pub use client::{Client, DEFAULT_BASE, DEFAULT_MODEL};
pub use types::{
  Content, Delta, Function, FunctionCall, Message, Model, Part, Request, Role, StreamEvent,
  ToolCall, ToolDef, Usage,
};
