//! Plain Markdown project knowledge for Asylum.
//!
//! A vault is an ordinary directory of Markdown files. This crate owns safe
//! file operations, Obsidian-compatible properties and wiki links, backlinks,
//! templates, note search, and durable links back to Asylum tasks and runs.

mod model;
mod parse;
mod search;
mod template;
mod vault;

pub use model::{Index, Link, Note, Property, Reference, ReferenceKind, Template};
pub use parse::{completion_fragment, parse, preview_source};
pub use search::{search, suggest, Hit};
pub use template::template;
pub use vault::{append_reference, create, delete, index, read, rename, write, Error, Result};

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
