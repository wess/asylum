//! Plain Markdown project knowledge for Asylum.
//!
//! A vault is an ordinary directory of Markdown files. This crate owns safe
//! file operations, YAML frontmatter properties and wiki links, backlinks,
//! templates, note search, and durable links back to Asylum tasks and runs.

mod model;
mod parse;
mod property;
mod search;
mod template;
mod vault;

pub use model::{Index, Link, Note, Property, Reference, ReferenceKind, Template};
pub use parse::{completion_fragment, parse, preview_source, preview_source_in};
pub use property::{remove_property, set_property};
pub use search::{search, suggest, Hit};
pub use template::{iso_date, iso_time, render_user_template, template};
pub use vault::{
    append_reference, create, create_from_template, delete, index, read, rename,
    save_user_template, user_templates, write, Error, Result, UserTemplate,
};

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
