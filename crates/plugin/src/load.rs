//! Discover and load plugins from a directory.

use std::path::{Path, PathBuf};

use crate::model::{Diagnostic, Plugin};
use crate::{parse, MANIFEST};

/// The result of scanning a plugins directory: the plugins that loaded plus
/// diagnostics for the ones that did not.
#[derive(Debug, Clone, Default)]
pub struct Installed {
    pub plugins: Vec<Plugin>,
    pub diagnostics: Vec<Diagnostic>,
}

/// The default plugins directory: `$XDG_DATA_HOME/asylum/plugins`, falling back
/// to `~/.local/share/asylum/plugins`.
pub fn default_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from(".local/share"));
    base.join("asylum").join("plugins")
}

/// Load every plugin under `dir` (each an immediate subdirectory holding a
/// `plugin.toml`). A missing directory is not an error - it yields empty.
pub fn load_dir(dir: &Path) -> Installed {
    let mut out = Installed::default();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return out,
        Err(e) => {
            out.diagnostics.push(Diagnostic {
                path: dir.to_path_buf(),
                message: format!("could not read plugins dir: {e}"),
            });
            return out;
        }
    };

    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();

    for plugin_dir in dirs {
        let manifest = plugin_dir.join(MANIFEST);
        if !manifest.exists() {
            continue;
        }
        match std::fs::read_to_string(&manifest) {
            Ok(text) => match parse(&text, plugin_dir.clone()) {
                Ok(plugin) => out.plugins.push(plugin),
                Err(message) => out.diagnostics.push(Diagnostic {
                    path: manifest,
                    message,
                }),
            },
            Err(e) => out.diagnostics.push(Diagnostic {
                path: manifest,
                message: format!("could not read manifest: {e}"),
            }),
        }
    }
    out
}

#[cfg(test)]
#[path = "../tests/load.rs"]
mod tests;
