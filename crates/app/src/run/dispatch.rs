//! Plugin event payloads and trigger dispatch for ADE events.

use std::path::PathBuf;
use std::time::Duration;

use gpui::Context;

use crate::state::Root;

/// How long a single plugin trigger invocation may run before it is abandoned
/// and its process killed, so a hung or slow plugin can never wedge the fleet.
const PLUGIN_TRIGGER_TIMEOUT: Duration = Duration::from_secs(30);

impl Root {
    /// An event payload for a run: fills task, project path, and worktree from
    /// the store when they can be resolved.
    pub(crate) fn run_event(&self, event: &str, run_id: i64) -> plugin::EventPayload {
        let mut payload = plugin::EventPayload::new(event).run(run_id);
        if let Ok(run) = self.db.run(run_id) {
            payload = payload.task(run.task_id).worktree(run.worktree);
            if let Ok(task) = self.db.task(run.task_id) {
                if let Ok(project) = self.db.project(task.project_id) {
                    payload = payload.project(project.path);
                }
            }
        }
        payload
    }

    /// An event payload for a task: fills the project path when resolvable.
    pub(crate) fn task_event(&self, event: &str, task_id: i64) -> plugin::EventPayload {
        let mut payload = plugin::EventPayload::new(event).task(task_id);
        if let Ok(task) = self.db.task(task_id) {
            if let Ok(project) = self.db.project(task.project_id) {
                payload = payload.project(project.path);
            }
        }
        payload
    }

    /// Fire every enabled plugin trigger hooked on `payload.event`. `notify`
    /// actions post to the Inbox; `invoke` actions run the plugin's runtime off
    /// the UI thread with a per-invocation timeout, and any failure posts an
    /// Inbox notice naming the plugin — never a crash, never silent. Disabled
    /// plugins never reach a runtime: the trust gate is enforced in
    /// [`plugin::fired`], and this returns immediately when nothing is enabled.
    pub(crate) fn dispatch_event(&mut self, payload: plugin::EventPayload, cx: &mut Context<Self>) {
        let enabled = self.settings.enabled_plugins.clone();
        if enabled.is_empty() {
            return;
        }
        let installed = self.plugins();
        let params = serde_json::to_value(&payload).unwrap_or(serde_json::Value::Null);
        // A process trigger runs in the event's worktree (or the project) so it
        // acts on the changes; WASM resolves its module against the plugin dir.
        let cwd = payload
            .worktree
            .as_deref()
            .or(payload.project.as_deref())
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        struct Invoke {
            name: String,
            run: Option<i64>,
            kind: plugin::RuntimeKind,
            runtime: plugin::Runtime,
            dir: PathBuf,
            caps: Vec<String>,
            method: String,
        }
        let mut notifies: Vec<(String, String)> = Vec::new();
        let mut invokes: Vec<Invoke> = Vec::new();
        for fired in plugin::fired(
            &installed.plugins,
            |id| enabled.iter().any(|e| e == id),
            &payload,
        ) {
            match &fired.trigger.action {
                plugin::TriggerAction::Notify { text } => {
                    notifies.push((fired.plugin.name.clone(), text.clone()));
                }
                plugin::TriggerAction::Invoke { method } => match &fired.plugin.runtime {
                    Some(runtime) => invokes.push(Invoke {
                        name: fired.plugin.name.clone(),
                        run: payload.run,
                        kind: runtime.kind,
                        runtime: runtime.clone(),
                        dir: fired.plugin.path.clone(),
                        caps: fired.plugin.capabilities.clone(),
                        method: method.clone(),
                    }),
                    None => notifies.push((
                        fired.plugin.name.clone(),
                        format!(
                            "A `{}` trigger wants to invoke `{method}`, but the plugin declares no [runtime].",
                            payload.event
                        ),
                    )),
                },
            }
        }

        let run_id = payload.run;
        for (name, text) in notifies {
            self.push_notification("plugin", &name, &text, run_id);
        }
        for invoke in invokes {
            let params = params.clone();
            let cwd = cwd.clone();
            let name = invoke.name.clone();
            let run = invoke.run;
            let method = invoke.method.clone();
            let job = cx.background_executor().spawn(async move {
                match invoke.kind {
                    plugin::RuntimeKind::Process => pluginrt::invoke_once_timeout(
                        &invoke.runtime,
                        &cwd,
                        &invoke.method,
                        params,
                        PLUGIN_TRIGGER_TIMEOUT,
                    ),
                    plugin::RuntimeKind::Wasm => pluginrt::invoke_wasm(
                        &invoke.runtime,
                        &invoke.dir,
                        &invoke.caps,
                        &invoke.method,
                        &params,
                    ),
                }
                .map(|_| ())
                .map_err(|e| e.to_string())
            });
            cx.spawn(async move |root, cx| {
                let result = job.await;
                let _ = root.update(cx, |root, cx| {
                    match result {
                        Ok(()) => {
                            tracing::info!(plugin = %name, method = %method, "plugin trigger ran")
                        }
                        Err(error) => root.push_notification(
                            "plugin",
                            &format!("{name} trigger failed"),
                            &error,
                            run,
                        ),
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }
}
