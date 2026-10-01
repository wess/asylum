//! A Model Context Protocol client. Plugins are MCP servers reached over a
//! child process's stdio (`Command`) or Streamable HTTP (`Remote HTTPS`).
//! `oauth` implements the MCP authorization flow for remote servers.

pub mod client;
pub mod http;
pub mod oauth;
pub mod rpc;
pub mod stdio;

pub use client::{Client, Tool};
pub use oauth::Tokens;
