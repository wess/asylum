//! The Bot runtime. `Runtime` owns the database, the shared computer, the
//! connected plugins, and a per-Bot work queue. Sending a message enqueues a
//! job; a job is a `turn` — the model streaming, calling tools, pausing for
//! approvals — whose progress is written to the store and announced as
//! `Event`s for the UI.

pub mod admin;
pub mod api;
pub mod approve;
pub mod audit;
pub mod catalog;
pub mod context;
pub mod event;
pub mod history;
pub mod media;
pub mod memory;
pub mod models;
pub mod otel;
pub mod part;
pub mod plugins;
pub mod policy;
pub mod prompt;
pub mod queue;
pub mod review;
pub mod route;
pub mod runtime;
pub mod teach;
pub mod template;
pub mod ticker;
pub mod tools;
pub mod turn;
pub mod usage;
pub mod webhook;

pub use event::Event;
pub use part::Part;
pub use queue::{Job, Origin};
pub use runtime::Runtime;
