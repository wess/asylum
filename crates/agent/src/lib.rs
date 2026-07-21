//! Agent registry, command building, and fan-out planning.
//!
//! The heart of the ADE is running *many* coding agents against one prompt,
//! each in its own isolated worktree, then comparing. This crate owns the
//! agent-facing half of that:
//!
//! - [`registry`] - the catalog of known CLI agents (Claude Code, Codex,
//!   OpenCode, …) and how each is launched, plus the vocabulary for how a prompt
//!   reaches an agent ([`Delivery`]).
//! - [`command`] - resolve an [`AgentDef`] + user [`config::AgentPrefs`] + a
//!   prompt + a working directory into a concrete [`SpawnSpec`] the host launches
//!   on a pty.
//! - [`plan`] - turn a task (a prompt + a set of agent ids) into a set of
//!   [`RunPlan`]s: one branch and worktree path per agent.
//!
//! The crate is pure and gpui-free: it never spawns a process itself. The app
//! layer takes a [`SpawnSpec`] and runs it inside an embedded terminal pane.

pub mod activity;
pub mod command;
pub mod doctor;
pub mod plan;
pub mod probe;
pub mod registry;

pub use activity::{classify, default_rules, rules_for, Activity, ActivityRules};
pub use command::SpawnSpec;
pub use plan::{slugify, RunPlan};
pub use registry::{builtins, catalog, find, resolve, Agent, AgentDef, Delivery};
