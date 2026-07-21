//! Installed plugins: the enable/disable trust gate and command invocation.

use crate::state::Root;

impl Root {
    /// Installed plugins (from the plugins directory) and any load diagnostics.
    pub fn plugins(&self) -> plugin::Installed {
        plugin::load_dir(&plugin::default_dir())
    }

    /// The plugins directory path, for display in the Plugins view.
    pub fn plugins_dir(&self) -> String {
        plugin::default_dir().to_string_lossy().into_owned()
    }

    /// Whether the user has enabled this plugin. A disabled plugin is inert: its
    /// triggers never fire and its commands never run. Everything is disabled by
    /// default, so a trusted process plugin can act only after a deliberate
    /// opt-in.
    pub fn plugin_enabled(&self, id: &str) -> bool {
        self.settings.enabled_plugins.iter().any(|e| e == id)
    }

    /// Begin enabling a plugin. A process runtime is fully trusted (it runs with
    /// the user's privileges and its capabilities are advisory), so route it
    /// through the confirm bar restating the exact command and its authority; a
    /// WASM or runtime-less plugin is capability-sandboxed and enables directly.
    pub fn request_enable_plugin(&mut self, id: &str, cx: &mut gpui::Context<Self>) {
        let installed = self.plugins();
        let Some(plugin) = installed.plugins.iter().find(|p| p.id == id) else {
            self.push_error(
                "Plugin unavailable",
                format!("{id} is no longer installed."),
            );
            return;
        };
        if let Some(runtime) = &plugin.runtime {
            if runtime.kind.is_trusted() {
                self.confirm = Some(crate::run::ConfirmAction::EnablePlugin {
                    id: plugin.id.clone(),
                    name: plugin.name.clone(),
                    disclosure: runtime.trust_summary(),
                });
                cx.notify();
                return;
            }
        }
        self.enable_plugin_now(id, cx);
    }

    /// Add a plugin to the enabled list and persist. Called directly for
    /// sandboxed plugins and after the trust confirmation for process plugins.
    pub fn enable_plugin_now(&mut self, id: &str, cx: &mut gpui::Context<Self>) {
        let mut list = self.settings.enabled_plugins.clone();
        if !list.iter().any(|e| e == id) {
            list.push(id.to_string());
        }
        self.write_enabled_plugins(list, cx);
    }

    /// Remove a plugin from the enabled list and persist. Its triggers stop
    /// firing and its commands stop running on the next event.
    pub fn disable_plugin(&mut self, id: &str, cx: &mut gpui::Context<Self>) {
        let list: Vec<String> = self
            .settings
            .enabled_plugins
            .iter()
            .filter(|e| e.as_str() != id)
            .cloned()
            .collect();
        self.write_enabled_plugins(list, cx);
    }

    /// Write the enabled-plugin list back to settings.json through the
    /// comment-preserving editor, then reload so the change applies live.
    fn write_enabled_plugins(&mut self, list: Vec<String>, cx: &mut gpui::Context<Self>) {
        let path = config::default_path();
        let result = if list.is_empty() {
            config::edit::remove_key(&path, "enabled_plugins")
        } else {
            config::edit::set_key(
                &path,
                "enabled_plugins",
                &serde_json::json!(list).to_string(),
            )
        };
        if let Err(e) = result {
            self.push_error("Could not save plugin settings", e.to_string());
            return;
        }
        crate::reload::reload(self, cx);
    }

    /// Invoke a plugin command through its declared runtime (process or WASM),
    /// passing the current project/task as context. Runs synchronously; the
    /// result (or error) surfaces as a notice.
    pub fn run_plugin_command(
        &mut self,
        plugin_id: &str,
        method: &str,
        cx: &mut gpui::Context<Self>,
    ) {
        let installed = self.plugins();
        let Some(plugin) = installed.plugins.iter().find(|p| p.id == plugin_id) else {
            self.push_error(
                "Plugin unavailable",
                format!("{plugin_id} is no longer installed."),
            );
            return;
        };
        if !self.plugin_enabled(plugin_id) {
            self.push_error(
                "Plugin not enabled",
                format!(
                    "Enable {} in the Plugins surface before running its commands.",
                    plugin.name
                ),
            );
            return;
        }
        let Some(runtime) = &plugin.runtime else {
            self.push_error(
                "Plugin has no runtime",
                format!(
                    "{} declares commands but no [runtime] to run them.",
                    plugin.name
                ),
            );
            return;
        };
        let project = self.project_id.and_then(|id| self.db.project(id).ok());
        let params = serde_json::json!({
            "command": method,
            "project": project.as_ref().map(|p| p.path.clone()),
            "task": self.task_id,
        });
        let cwd = project
            .as_ref()
            .map(|p| std::path::PathBuf::from(&p.path))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let result = match runtime.kind {
            plugin::RuntimeKind::Process => pluginrt::invoke_once(runtime, &cwd, method, params),
            plugin::RuntimeKind::Wasm => {
                pluginrt::invoke_wasm(runtime, &plugin.path, &plugin.capabilities, method, &params)
            }
        };
        match result {
            Ok(value) => {
                let summary: String = value.to_string().chars().take(200).collect();
                self.push_notice(
                    crate::run::NoticeTone::Success,
                    format!("{} ran", plugin.name),
                    summary,
                );
            }
            Err(error) => self.push_error("Plugin command failed", error.to_string()),
        }
        cx.notify();
    }
}
