//! Launch queued runs onto real ptys and stream their output to the store.

use std::path::{Path, PathBuf};

use gpui::{App, AppContext as _, Context, Entity, Window};
use libsinclair::terminal::{Event, SessionOptions};
use libsinclair::termview::{TermOptions, TermView};

use super::persist;
use crate::state::{now, Root};

pub fn terminal_text(term: &Entity<TermView>, cx: &App) -> String {
    term.read(cx).session().with_term(|terminal| {
        terminal
            .text_lines()
            .into_iter()
            .map(|(_, line, _)| line)
            .collect::<Vec<_>>()
            .join("\n")
            .trim_end()
            .to_string()
    })
}

/// Prepare a raw transcript for the store: mask any known secret that leaked
/// into the terminal (e.g. an upstream that echoes a credential), then cap it to
/// the storage budget. Every persisted transcript — the lazy snapshots here and
/// the final flush at a terminal transition in `lifecycle` — goes through this
/// so the row is uniformly masked and bounded.
pub(crate) fn redact_and_cap(raw: &str) -> String {
    persist::cap(&crate::secrets::redact(raw))
}

impl Root {
    pub fn launch_queued(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.launch_needed = false;
        let running = match self.db.running_count() {
            Ok(count) => count,
            Err(error) => {
                self.push_error("Could not inspect running agents", error.to_string());
                return;
            }
        };
        let limit = if self.settings.max_parallel_runs == 0 {
            usize::MAX
        } else {
            self.settings.max_parallel_runs as usize
        };
        let capacity = limit.saturating_sub(running);
        if capacity == 0 {
            return;
        }
        let queued = match self.db.queued_runs() {
            Ok(runs) => runs,
            Err(error) => {
                self.push_error("Could not read run queue", error.to_string());
                return;
            }
        };
        for run in queued.into_iter().take(capacity) {
            self.launch_run(run.id, window, cx);
        }
    }

    fn launch_run(&mut self, run_id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let Ok(run) = self.db.run(run_id) else {
            self.push_error(
                "Run unavailable",
                format!("Queued run {run_id} could not be loaded."),
            );
            return;
        };
        let Ok(task) = self.db.task(run.task_id) else {
            self.fail_launch(run_id, "The task for this run no longer exists.");
            return;
        };
        let Ok(project) = self.db.project(task.project_id) else {
            self.fail_launch(run_id, "The project for this run no longer exists.");
            return;
        };
        let Some(agent) = agent::registry::resolve(&run.agent, &self.settings.custom_agents) else {
            self.fail_launch(run_id, "The configured agent no longer exists.");
            return;
        };
        let mut prompt = run.prompt.clone().unwrap_or(task.prompt);
        prompt.push_str(&self.note_context_for_run(run_id));
        let prefs = self.settings.agents.get(&run.agent);
        let mut spec = agent::command::build(&agent, prefs, &prompt, &run.worktree);
        if agent::doctor::find_program(&spec.program).is_none() {
            self.fail_launch(run_id, &format!("{} was not found on PATH.", spec.program));
            return;
        }
        spec.env = self.control_env(run.task_id, run_id);
        let project_config = config::load_project(Path::new(&project.path)).0;
        let pidfile = crate::reap::pidfile(run_id);
        let _ = std::fs::remove_file(&pidfile);
        let term = match make_term(spec.clone(), project_config.env, &pidfile, window, cx) {
            Ok(term) => term,
            Err(error) => {
                self.fail_launch(run_id, &format!("Could not start {}: {error}", agent.name));
                return;
            }
        };
        if let Some(stdin) = spec.stdin {
            let write = term
                .read(cx)
                .session()
                .write(stdin.as_bytes())
                .and_then(|_| term.read(cx).session().write(b"\n"));
            if let Err(error) = write {
                crate::reap::terminate(&pidfile);
                self.fail_launch(
                    run_id,
                    &format!("Could not send the prompt to {}: {error}", agent.name),
                );
                return;
            }
        }
        if let Err(error) = self.db.start_run(run_id, now()) {
            crate::reap::terminate(&pidfile);
            self.push_error("Could not start run", error.to_string());
            self.fail_launch(
                run_id,
                "The run could not be marked as started in the workspace store.",
            );
            return;
        }
        let _ = self.db.set_run_activity(run_id, Some("working"));
        let _ = self.db.record_event(
            "run_started",
            Some(run.task_id),
            Some(run_id),
            &format!("{{\"agent\":\"{}\"}}", run.agent),
            now(),
        );
        tracing::info!(run_id, agent = %run.agent, "run launched");
        let started = self.run_event("run_started", run_id).status("started");
        self.dispatch_event(started, cx);
        cx.subscribe(&term, move |root, term, event: &Event, cx| match event {
            Event::Wakeup => root.snapshot_run(run_id, &term, cx),
            Event::Exit(code) => root.finish_run(run_id, *code, &term, cx),
            _ => {}
        })
        .detach();
        self.run_terms.insert(run_id, term);
        self.run_pidfiles.insert(run_id, pidfile);
        self.arm_timeout(run_id, window, cx);
    }

    fn fail_launch(&mut self, run_id: i64, message: &str) {
        tracing::warn!(run_id, reason = message, "run launch failed");
        if let Err(error) = self.db.fail_run(run_id, message, "", now()) {
            self.push_error("Could not record launch failure", error.to_string());
        }
        self.push_error(
            "Agent launch failed",
            format!("{message} Retry after fixing the setup."),
        );
        self.refresh_task_for_run(run_id);
        self.launch_needed = true;
    }

    fn snapshot_run(&mut self, run_id: i64, term: &Entity<TermView>, cx: &App) {
        let now = now();
        // Throttle every tick — the transcript rebuild and activity
        // reclassification — to at most once per second.
        if self.run_saved_at.get(&run_id) == Some(&now) {
            return;
        }
        self.run_saved_at.insert(run_id, now);

        // Rebuild the live transcript once and reclassify activity every tick:
        // the board's "who needs me" signal has to stay fresh, and it reads the
        // in-memory text, never the store.
        let text = terminal_text(term, cx);
        self.update_activity(run_id, &text);

        // Persistence is lazy. The open pane renders the live `TermView`, so the
        // stored transcript only backs restart recovery and finished-run
        // history: write it on a slow cadence, only when it actually changed,
        // and capped so a runaway agent cannot bloat the row. A terminal
        // transition (finish/cancel/timeout) still flushes a final capped
        // snapshot from `lifecycle`.
        if !persist::due(run_id, &text, now) {
            return;
        }
        match self.db.save_run_output(run_id, &redact_and_cap(&text)) {
            Ok(()) => {
                self.run_save_failed.remove(&run_id);
                persist::record(run_id, &text, now);
            }
            Err(error) if self.run_save_failed.insert(run_id) => {
                self.push_error(
                    "Terminal output is not being saved",
                    format!("{error}. The live terminal remains available, but restart recovery is at risk."),
                );
            }
            Err(_) => {}
        }
    }

    /// Environment variables injected into a launched agent: the control surface
    /// (orchestrate the fleet) and the secrets proxy (masked outbound API calls).
    /// Each is added only when its server is enabled.
    fn control_env(&self, task_id: i64, run_id: i64) -> Vec<(String, String)> {
        let mut env = Vec::new();

        if self.settings.control.enabled {
            let port = self
                .settings
                .control
                .bind
                .rsplit(':')
                .next()
                .unwrap_or("8788");
            // Mint a scoped token bound to this run's task and signed with the
            // per-session key (see `secrets`), so an agent can orchestrate its own
            // fleet but a request for another task is refused. An empty key means
            // the control server runs unauthenticated (auth disabled).
            let key = crate::secrets::control_token();
            let token = if key.is_empty() {
                String::new()
            } else {
                // Comfortably longer than any run; the key rotates each session.
                let expires_at = now() + 7 * 24 * 60 * 60;
                ::control::token::mint(&key, task_id, run_id, expires_at)
            };
            env.push((
                ::control::ENV_URL.to_string(),
                format!("http://127.0.0.1:{port}"),
            ));
            env.push((::control::ENV_TOKEN.to_string(), token));
            env.push((::control::ENV_TASK.to_string(), task_id.to_string()));
            env.push((::control::ENV_RUN.to_string(), run_id.to_string()));
        }

        // The secrets proxy: the agent reaches it at 127.0.0.1:<port> and calls
        // named upstreams without ever seeing the credentials (`asylum call`).
        // The token is signed with the session key and names this run's project,
        // so the proxy resolves secrets from the project's keep (overlaid on
        // global) and can't be tricked into another project's scope.
        if self.settings.proxy.enabled {
            let port = self
                .settings
                .proxy
                .bind
                .rsplit(':')
                .next()
                .unwrap_or("8789");
            let key = crate::secrets::proxy_key();
            // A run whose task will not resolve gets no token rather than a
            // project-0 one: falling back to global scope would quietly hand it
            // the global keep. Proxy access is granted, never defaulted into.
            let token = match self.db.task(task_id) {
                Ok(t) if !key.is_empty() => {
                    let expires_at = now() + 7 * 24 * 60 * 60;
                    proxy::token::mint(&key, t.project_id, expires_at)
                }
                _ => String::new(),
            };
            env.push((
                proxy::ENV_URL.to_string(),
                format!("http://127.0.0.1:{port}"),
            ));
            env.push((proxy::ENV_TOKEN.to_string(), token));
        }

        // The MCP gateway: the agent connects to one aggregated MCP server at
        // 127.0.0.1:<port>/mcp and sees every configured service's tools,
        // namespaced `<service>__<tool>`. The token names this run's project
        // (which servers it may see) and its run id (for attribution), signed
        // with the session key. As with the proxy, a run whose task will not
        // resolve gets no token rather than a project-0 one.
        if self.settings.mcp.enabled {
            let port = self.settings.mcp.bind.rsplit(':').next().unwrap_or("8790");
            let key = crate::secrets::mcp_key();
            let token = match self.db.task(task_id) {
                Ok(t) if !key.is_empty() => {
                    let expires_at = now() + 7 * 24 * 60 * 60;
                    mcp::token::mint(&key, t.project_id, run_id, expires_at)
                }
                _ => String::new(),
            };
            env.push((mcp::ENV_URL.to_string(), format!("http://127.0.0.1:{port}")));
            env.push((mcp::ENV_TOKEN.to_string(), token));
        }

        env
    }

    /// Classify a run's live output into a semantic activity and persist it when
    /// it changes, emitting a `run_activity` event so the board, the phone, and
    /// sibling agents see the transition. A `None` classification keeps the
    /// prior state (avoids flapping to idle between bursts of output).
    fn update_activity(&mut self, run_id: i64, text: &str) {
        let Ok(run) = self.db.run(run_id) else { return };
        let Some(activity) = agent::Activity::detect(&run.agent, text) else {
            return;
        };
        let token = activity.as_str();
        if run.activity.as_deref() == Some(token) {
            return;
        }
        let _ = self.db.set_run_activity(run_id, Some(token));
        let _ = self.db.record_event(
            "run_activity",
            Some(run.task_id),
            Some(run_id),
            &format!("{{\"activity\":\"{token}\"}}"),
            now(),
        );
    }
}

fn make_term(
    spec: agent::SpawnSpec,
    env: std::collections::BTreeMap<String, String>,
    pidfile: &Path,
    window: &mut Window,
    cx: &mut Context<Root>,
) -> std::io::Result<Entity<TermView>> {
    // Launch through the pid-recording wrapper so the agent's process group
    // can be ended deterministically later (see `reap`).
    let argv = std::iter::once(spec.program).chain(spec.args).collect();
    let mut options = SessionOptions::command(crate::reap::wrap(argv, pidfile));
    options.spawn.cwd = Some(PathBuf::from(spec.cwd));
    options.spawn.env.extend(env);
    // Control-surface variables (ASYLUM_RUN_ID, …) so the agent can orchestrate.
    options.spawn.env.extend(spec.env);
    let (session, events) = libsinclair::Session::spawn(options)?;
    let session = std::sync::Arc::new(session);
    Ok(cx.new(|cx| TermView::new(session, events, TermOptions::default(), window, cx)))
}
