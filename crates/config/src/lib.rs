//! Settings for the app. The source of truth is `agents.json` (JSON with
//! comments): compiled-in defaults overridden by whatever keys the user's file
//! sets. Secrets never live in that file; `secret` keeps them in the OS
//! keychain.

pub mod jsonc;
pub mod paths;
pub mod policy;
pub mod provider;
pub mod secret;
pub mod settings;
pub mod watch;

pub use provider::{Credential, Kind, Output, Profile};
pub use policy::Policy;
pub use paths::{config_dir, data_dir, settings_path};
pub use settings::{load, save, Appearance, Approval, Load, LocalExec, Settings};
pub use watch::{watch, WatchHandle};

pub const APP: &str = "asylum";
