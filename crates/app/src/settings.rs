//! The Settings surface: a Zed-style settings editor over settings.json.
//!
//! The file is the single source of truth. Every control writes one key
//! through `config::edit` (the user's comments and formatting survive), then
//! re-loads and applies the result; edits made outside the app arrive through
//! the live-reload watcher the same way. A modified setting shows a Reset
//! that removes its key, restoring the built-in default.

use gpui::prelude::*;
use gpui::{div, px, App, Context, Entity, IntoElement, SharedString, Window};
use guise::prelude::*;
use guise::{Kbd, TextInputEvent};
use serde_json::json;

use crate::control::{Button, Switch};
use crate::state::Root;

/// A running or finished CLI probe for one agent.
#[derive(Clone)]
pub enum Test {
    Running,
    Done(agent::doctor::Probe),
}

/// One row of the Agents section: the agent (with the user's executable
/// override resolved), whether it is installed, and its last CLI probe.
#[derive(Clone)]
pub struct AgentRow {
    pub agent: agent::registry::Agent,
    pub report: agent::doctor::Report,
    pub test: Option<Test>,
}

/// Run one agent's CLI off-thread and record what it said for the row to show.
/// The probe uses the configured executable, so a corrected path is tested as
/// typed rather than whatever is on PATH.
pub fn test_agent(root: &mut Root, id: String, program: String, cx: &mut Context<Root>) {
    root.agent_tests.insert(id.clone(), Test::Running);
    cx.notify();
    let executor = cx.background_executor().clone();
    cx.spawn(async move |handle, cx| {
        let probe = executor
            .spawn(async move { agent::doctor::probe(&program) })
            .await;
        handle
            .update(cx, |root, cx| {
                root.agent_tests.insert(id, Test::Done(probe));
                cx.notify();
            })
            .ok();
    })
    .detach();
}

/// The surface's text inputs, built lazily and kept in sync with the file.
///
/// The single-value inputs are seeded once and mirrored on reload; the list
/// editors (`*_form`) hold the fields of whichever add/edit form is open and
/// are cleared when it is saved or cancelled. All of this lives here rather
/// than on `Root` so the Settings surface owns its own transient state.
#[derive(Clone)]
pub struct Inputs {
    pub worktree: Entity<guise::TextInput>,
    pub font: Entity<guise::TextInput>,
    pub companion_bind: Entity<guise::TextInput>,
    pub control_bind: Entity<guise::TextInput>,
    pub proxy_bind: Entity<guise::TextInput>,
    pub mcp_bind: Entity<guise::TextInput>,
    pub programs: std::collections::BTreeMap<String, Entity<guise::TextInput>>,
    /// The keep passphrase field (masked); Unlock reads it, then clears it.
    pub keep_pass: Entity<guise::TextInput>,
    /// The last keep unlock/create error, shown inline until the next attempt.
    pub keep_error: Option<SharedString>,
    /// The open add-secret form, if any.
    pub secret_form: Option<SecretForm>,
    /// The open MCP-server editor, if any.
    pub server_form: Option<ServerForm>,
    /// The open proxy-upstream editor, if any.
    pub upstream_form: Option<UpstreamForm>,
    /// The open custom-agent editor, if any.
    pub agent_form: Option<CustomAgentForm>,
    /// The open fan-out layout editor, if any.
    pub layout_form: Option<LayoutForm>,
}

/// The add-secret form: a name, a masked value, and the scope to store it in.
/// The value is only ever read on save and handed straight to the keep - it is
/// never echoed back and never leaves memory.
#[derive(Clone)]
pub struct SecretForm {
    /// Scope to store into: `0` = global, else a project id.
    pub project: i64,
    /// The project the surface is focused on, offered as "This project".
    pub current_project: Option<i64>,
    pub name: Entity<guise::TextInput>,
    pub value: Entity<guise::TextInput>,
    pub error: Option<SharedString>,
}

/// The MCP-server editor. `index` is the row being edited (`None` = adding);
/// `base` carries the fields the form does not expose (env, header, format) so
/// an edit preserves them.
#[derive(Clone)]
pub struct ServerForm {
    pub index: Option<usize>,
    pub base: config::McpServer,
    pub transport: String,
    pub project: i64,
    pub current_project: Option<i64>,
    pub enabled: bool,
    pub name: Entity<guise::TextInput>,
    pub command: Entity<guise::TextInput>,
    pub args: Entity<guise::TextInput>,
    pub url: Entity<guise::TextInput>,
    pub secret: Entity<guise::TextInput>,
    pub allow: Entity<guise::TextInput>,
    pub deny: Entity<guise::TextInput>,
    pub error: Option<SharedString>,
}

/// The proxy-upstream editor. `base` preserves the header/format the form does
/// not expose.
#[derive(Clone)]
pub struct UpstreamForm {
    pub index: Option<usize>,
    pub base: config::Upstream,
    pub project: i64,
    pub current_project: Option<i64>,
    pub name: Entity<guise::TextInput>,
    pub base_url: Entity<guise::TextInput>,
    pub secret: Entity<guise::TextInput>,
    pub error: Option<SharedString>,
}

/// The custom-agent editor.
#[derive(Clone)]
pub struct CustomAgentForm {
    pub index: Option<usize>,
    pub delivery: String,
    pub id: Entity<guise::TextInput>,
    pub name: Entity<guise::TextInput>,
    pub icon: Entity<guise::TextInput>,
    pub program: Entity<guise::TextInput>,
    pub args: Entity<guise::TextInput>,
    pub error: Option<SharedString>,
}

/// The fan-out layout editor.
#[derive(Clone)]
pub struct LayoutForm {
    pub index: Option<usize>,
    pub concurrency: u32,
    pub name: Entity<guise::TextInput>,
    pub description: Entity<guise::TextInput>,
    pub agents: Entity<guise::TextInput>,
    pub error: Option<SharedString>,
}

/// Make sure the text inputs exist; submitting one writes its key.
pub fn ensure_inputs(root: &mut Root, cx: &mut Context<Root>) {
    if root.settings_inputs.is_some() {
        return;
    }
    let worktree = text_input(
        cx,
        &root.settings.worktree_dir,
        "worktree_dir",
        |root, text, cx| {
            let text = text.trim();
            if text.is_empty() || text == config::Settings::default().worktree_dir {
                reset(root, "worktree_dir", cx);
            } else {
                write(root, "worktree_dir", json!(text), cx);
            }
        },
    );
    let font = text_input(
        cx,
        &root.settings.editor.font_family,
        "font_family",
        |root, text, cx| {
            let mut editor = root.settings.editor.clone();
            editor.font_family = if text.trim().is_empty() {
                config::EditorPrefs::default().font_family
            } else {
                text.trim().to_string()
            };
            write_editor(root, editor, cx);
        },
    );
    let companion_bind = text_input(
        cx,
        &root.settings.companion.bind,
        "127.0.0.1:8787",
        |root, text, cx| {
            let bind = if text.trim().is_empty() {
                config::CompanionPrefs::default().bind
            } else {
                text.trim().to_string()
            };
            write_companion(root, root.settings.companion.enabled, &bind, cx);
        },
    );
    let control_bind = text_input(
        cx,
        &root.settings.control.bind,
        "127.0.0.1:8788",
        |root, text, cx| {
            let bind = if text.trim().is_empty() {
                config::ControlPrefs::default().bind
            } else {
                text.trim().to_string()
            };
            write_control(root, root.settings.control.enabled, &bind, cx);
        },
    );
    let proxy_bind = text_input(
        cx,
        &root.settings.proxy.bind,
        "127.0.0.1:8789",
        |root, text, cx| {
            let bind = if text.trim().is_empty() {
                config::ProxyPrefs::default().bind
            } else {
                text.trim().to_string()
            };
            write(
                root,
                "proxy",
                json!({ "enabled": root.settings.proxy.enabled, "bind": bind }),
                cx,
            );
        },
    );
    let mcp_bind = text_input(
        cx,
        &root.settings.mcp.bind,
        "127.0.0.1:8790",
        |root, text, cx| {
            let bind = if text.trim().is_empty() {
                config::McpPrefs::default().bind
            } else {
                text.trim().to_string()
            };
            write_mcp(
                root,
                root.settings.mcp.enabled,
                &bind,
                &root.settings.mcp.expose.clone(),
                cx,
            );
        },
    );
    let mut programs = std::collections::BTreeMap::new();
    for agent in agent::registry::catalog(&root.settings.custom_agents) {
        let id = agent.id.clone();
        let default_program = agent.program.clone();
        let value = root
            .settings
            .agents
            .get(&id)
            .and_then(|prefs| prefs.program.clone())
            .unwrap_or_else(|| default_program.clone());
        let input = text_input(cx, &value, "executable", move |root, text, cx| {
            let mut agents = root.settings.agents.clone();
            let mut prefs = agents.get(&id).cloned().unwrap_or_default();
            let value = text.trim();
            prefs.program = if value.is_empty() || value == default_program {
                None
            } else {
                Some(value.to_string())
            };
            if prefs == config::AgentPrefs::default() {
                agents.remove(&id);
            } else {
                agents.insert(id.clone(), prefs);
            }
            write_agents(root, agents, cx);
        });
        programs.insert(agent.id, input);
    }
    let keep_pass = cx.new(|cx| {
        guise::TextInput::new(cx)
            .placeholder("keep passphrase")
            .password(true)
    });
    root.settings_inputs = Some(Inputs {
        worktree,
        font,
        companion_bind,
        control_bind,
        proxy_bind,
        mcp_bind,
        programs,
        keep_pass,
        keep_error: None,
        secret_form: None,
        server_form: None,
        upstream_form: None,
        agent_form: None,
        layout_form: None,
    });
}

/// A plain form field seeded with `value`. Unlike [`text_input`], it carries no
/// Submit wiring: a form's Save button reads every field at once.
fn field(
    cx: &mut Context<Root>,
    value: &str,
    placeholder: &'static str,
    password: bool,
) -> Entity<guise::TextInput> {
    cx.new(|cx| {
        let mut i = guise::TextInput::new(cx)
            .placeholder(placeholder)
            .password(password);
        i.set_text(value, cx);
        i
    })
}

/// A text input seeded with `value` whose Submit runs `commit` on the root.
fn text_input(
    cx: &mut Context<Root>,
    value: &str,
    placeholder: &'static str,
    commit: impl Fn(&mut Root, &str, &mut Context<Root>) + 'static,
) -> Entity<guise::TextInput> {
    let input = cx.new(|cx| {
        let mut i = guise::TextInput::new(cx).placeholder(placeholder);
        i.set_text(value, cx);
        i
    });
    cx.subscribe(&input, move |root, _input, event: &TextInputEvent, cx| {
        if let TextInputEvent::Submit(text) = event {
            commit(root, text, cx);
        }
    })
    .detach();
    input
}

/// Mirror externally-changed values into the inputs (no-op while they match).
pub fn sync_inputs(root: &mut Root, settings: &config::Settings, cx: &mut Context<Root>) {
    let Some(inputs) = &root.settings_inputs else {
        return;
    };
    for (input, value) in [
        (inputs.worktree.clone(), settings.worktree_dir.clone()),
        (inputs.font.clone(), settings.editor.font_family.clone()),
        (
            inputs.companion_bind.clone(),
            settings.companion.bind.clone(),
        ),
        (inputs.control_bind.clone(), settings.control.bind.clone()),
        (inputs.proxy_bind.clone(), settings.proxy.bind.clone()),
        (inputs.mcp_bind.clone(), settings.mcp.bind.clone()),
    ] {
        input.update(cx, |i, cx| {
            if i.text() != value {
                i.set_text(&value, cx);
            }
        });
    }
    let catalog = agent::registry::catalog(&settings.custom_agents);
    for agent in catalog {
        let Some(input) = inputs.programs.get(&agent.id) else {
            continue;
        };
        let value = settings
            .agents
            .get(&agent.id)
            .and_then(|prefs| prefs.program.clone())
            .unwrap_or(agent.program);
        input.update(cx, |input, cx| {
            if input.text() != value {
                input.set_text(&value, cx);
            }
        });
    }
}

// ── Writing back ────────────────────────────────────────────────────────────

/// Set one top-level settings.json key and apply the result immediately.
fn write(root: &mut Root, key: &str, value: serde_json::Value, cx: &mut Context<Root>) {
    if let Err(e) = config::edit::set_key(&config::default_path(), key, &value.to_string()) {
        root.push_error("Could not save settings", e.to_string());
        return;
    }
    crate::reload::reload(root, cx);
}

/// Remove one top-level key (back to the built-in default) and apply.
fn reset(root: &mut Root, key: &str, cx: &mut Context<Root>) {
    if let Err(e) = config::edit::remove_key(&config::default_path(), key) {
        root.push_error("Could not reset setting", e.to_string());
        return;
    }
    crate::reload::reload(root, cx);
}

fn write_agents(
    root: &mut Root,
    agents: std::collections::BTreeMap<String, config::AgentPrefs>,
    cx: &mut Context<Root>,
) {
    if agents.is_empty() {
        reset(root, "agents", cx);
    } else {
        write(root, "agents", json!(agents), cx);
    }
}

/// Persist editor prefs: the whole `editor` object, or nothing when it's all
/// defaults again (keeps the file minimal).
fn write_editor(root: &mut Root, editor: config::EditorPrefs, cx: &mut Context<Root>) {
    if editor == config::EditorPrefs::default() {
        reset(root, "editor", cx);
    } else if let Ok(value) = serde_json::to_value(&editor) {
        write(root, "editor", value, cx);
    }
}

/// Persist the `mcp` gateway prefs object, or drop the key when it is all
/// defaults again (keeps the file minimal). The `mcp_servers` list itself is
/// edited in settings.json - only the gateway toggles live here.
fn write_mcp(root: &mut Root, enabled: bool, bind: &str, expose: &str, cx: &mut Context<Root>) {
    let defaults = config::McpPrefs::default();
    if !enabled && bind == defaults.bind && expose == defaults.expose {
        reset(root, "mcp", cx);
    } else {
        write(
            root,
            "mcp",
            json!({ "enabled": enabled, "bind": bind, "expose": expose }),
            cx,
        );
    }
}

/// The `companion.token` value literally written in settings.json right now -
/// never `root.settings.companion.token`, which may have been filled in from
/// `ASYLUM_COMPANION_TOKEN` (see `config::load::resolve_secrets`). Reading the
/// file fresh keeps a Servers toggle from ever baking an environment-sourced
/// token into the file, or blanking a token the user put there directly.
fn companion_token_on_disk() -> String {
    std::fs::read_to_string(config::default_path())
        .map(|text| config::load_str(&text).settings.companion.token)
        .unwrap_or_default()
}

/// Persist the `companion` mobile-server prefs object, or drop the key when it
/// is all defaults again. `token` is never set from here - it round-trips the
/// on-disk value untouched so flipping Enable or editing Bind can never write
/// an environment-sourced token to disk or erase one the user configured.
fn write_companion(root: &mut Root, enabled: bool, bind: &str, cx: &mut Context<Root>) {
    let defaults = config::CompanionPrefs::default();
    let token = companion_token_on_disk();
    if !enabled && bind == defaults.bind && token.is_empty() {
        reset(root, "companion", cx);
    } else {
        write(
            root,
            "companion",
            json!({ "enabled": enabled, "bind": bind, "token": token }),
            cx,
        );
    }
}

/// Persist the `control` agent-surface prefs object, or drop the key when it is
/// all defaults again. `control.token` is never resolved from the environment
/// (unlike `companion.token`), so `root.settings.control.token` is always the
/// on-disk value and is safe to round-trip directly.
fn write_control(root: &mut Root, enabled: bool, bind: &str, cx: &mut Context<Root>) {
    let defaults = config::ControlPrefs::default();
    let token = root.settings.control.token.clone();
    if enabled == defaults.enabled && bind == defaults.bind && token.is_empty() {
        reset(root, "control", cx);
    } else {
        write(
            root,
            "control",
            json!({ "enabled": enabled, "bind": bind, "token": token }),
            cx,
        );
    }
}

// ── The surface ─────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn settings_view(
    settings: config::Settings,
    diagnostics: Vec<config::Diagnostic>,
    agents: Vec<AgentRow>,
    inputs: Inputs,
    collapsed: std::collections::HashSet<&'static str>,
    handle: Entity<Root>,
    _window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let t = guise::theme::theme(cx);
    let dimmed = t.dimmed().hsla();
    let border = t.border().hsla();
    let chrome = Chrome {
        border,
        dimmed,
        desc: px(t.font_size(Size::Xs)),
    };
    let defaults = config::Settings::default();
    let ed_defaults = config::EditorPrefs::default();
    let ed = settings.editor.clone();

    let mut col = div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(760.0))
        .gap_1()
        .p(px(20.0))
        .pb(px(40.0));

    // Header: title + the escape hatch to the raw file.
    let raw = handle.clone();
    col = col.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(Title::new("Settings").order(2))
            .child(
                Button::new("settings-open-file", "Edit in settings.json")
                    .variant(Variant::Default)
                    .size(Size::Xs)
                    .on_click(move |_, _, cx| {
                        let path = config::default_path();
                        if let Err(e) = config::edit::ensure_file(&path) {
                            raw.update(cx, |root, cx| {
                                root.push_error("Could not open settings", e.to_string());
                                cx.notify();
                            });
                            return;
                        }
                        cx.open_with_system(&path);
                    }),
            ),
    );
    col = col.child(
        Text::new(SharedString::from(format!(
            "{} — edits apply live, in the file and here.",
            config::default_path().display()
        )))
        .size(Size::Xs)
        .dimmed(),
    );

    // Problems from the last load, if any.
    for d in &diagnostics {
        let msg = if d.key.is_empty() {
            d.message.clone()
        } else {
            format!("{}: {}", d.key, d.message)
        };
        col = col.child(
            div().pt(px(8.0)).child(
                Badge::new(SharedString::from(msg))
                    .color(ColorName::Yellow)
                    .variant(Variant::Light),
            ),
        );
    }

    // Each group below is a collapsible accordion section: clicking its header
    // toggles the section's collapsed state (held on Root), so the page opens
    // compact and you scroll only the groups you expand.

    // ── General ──
    let mut body = section_body();
    body = body.child(row(
        "Theme",
        "Color scheme for the whole app.",
        settings.theme != defaults.theme,
        Some(reset_btn("reset-theme", "theme", &handle)),
        theme_control(&settings.theme, &handle),
        chrome,
    ));
    body = body.child(row(
        "Worktree directory",
        "Where per-task worktrees are created, relative to a project root. Press enter to apply.",
        settings.worktree_dir != defaults.worktree_dir,
        Some(reset_btn("reset-worktree", "worktree_dir", &handle)),
        div()
            .w(px(280.0))
            .flex_none()
            .child(inputs.worktree.clone())
            .into_any_element(),
        chrome,
    ));
    {
        let display = if settings.max_parallel_runs == 0 {
            "unlimited".to_string()
        } else {
            settings.max_parallel_runs.to_string()
        };
        let value = settings.max_parallel_runs as i64;
        body = body.child(row(
            "Max parallel runs",
            "Concurrent agent runs across all tasks; 0 means unlimited.",
            settings.max_parallel_runs != defaults.max_parallel_runs,
            Some(reset_btn("reset-parallel", "max_parallel_runs", &handle)),
            stepper(
                "parallel",
                value,
                (0, 32),
                display,
                border,
                &handle,
                |root, v, cx| {
                    write(root, "max_parallel_runs", json!(v), cx);
                },
            ),
            chrome,
        ));
    }
    {
        let display = if settings.run_timeout_minutes == 0 {
            "off".to_string()
        } else {
            format!("{} min", settings.run_timeout_minutes)
        };
        let value = settings.run_timeout_minutes as i64;
        body = body.child(row(
            "Run timeout",
            "Stop an agent that exceeds this duration; 0 disables the timeout.",
            settings.run_timeout_minutes != defaults.run_timeout_minutes,
            Some(reset_btn("reset-timeout", "run_timeout_minutes", &handle)),
            stepper(
                "timeout",
                value,
                (0, 240),
                display,
                border,
                &handle,
                |root, v, cx| {
                    write(root, "run_timeout_minutes", json!(v), cx);
                },
            ),
            chrome,
        ));
    }
    col = col.child(section(
        "general", "General", &collapsed, body, &handle, dimmed, border,
    ));

    // ── Agents ──
    let mut body = section_body();
    body = body.child(
        Text::new("Agents fanned out by default when a task is dispatched.")
            .size(Size::Xs)
            .dimmed(),
    );
    let selected = settings.default_agents.clone();
    for entry in &agents {
        let agent = entry.agent.clone();
        let on = selected.contains(&agent.id);
        let id = agent.id.clone();
        let h = handle.clone();
        let current = selected.clone();
        let (status, color) = if entry.report.verified() {
            ("verified", ColorName::Green)
        } else if entry.report.ready() {
            ("installed", ColorName::Orange)
        } else {
            ("not found", ColorName::Red)
        };
        let program = inputs.programs.get(&agent.id).cloned();
        let mut controls = div().flex().flex_row().items_center().gap_2();
        controls = controls.child(test_result(entry.test.as_ref()));
        controls = controls.child(Badge::new(status).color(color).variant(Variant::Light));
        if let Some(program) = program {
            controls = controls.child(
                div()
                    .id(SharedString::from(format!("program-tip-{}", agent.id)))
                    .w(px(190.0))
                    .tooltip(guise::tooltip(
                        "Executable name or absolute path. Press enter to apply.",
                    ))
                    .child(program),
            );
        }
        controls = controls.child(test_btn(&agent, &handle));
        controls = controls.child(
            Switch::new(SharedString::from(format!("agent-{}", agent.id)))
                .checked(on)
                .aria_label(SharedString::from(format!("Use {} by default", agent.name)))
                .size(Size::Sm)
                .on_change(move |_, _, cx| {
                    let mut next = current.clone();
                    if on {
                        next.retain(|a| a != &id);
                    } else {
                        next.push(id.clone());
                    }
                    h.update(cx, |root, cx| {
                        if next.is_empty() {
                            reset(root, "default_agents", cx);
                        } else {
                            write(root, "default_agents", json!(next), cx);
                        }
                        root.fanout = next.clone();
                    });
                }),
        );
        body = body.child(row(
            SharedString::from(agent.name.clone()),
            SharedString::from(format!("id: {} · executable", agent.id)),
            false,
            None,
            controls.into_any_element(),
            chrome,
        ));
    }
    col = col.child(section(
        "agents", "Agents", &collapsed, body, &handle, dimmed, border,
    ));

    // ── Custom agents ──
    col = col.child(section(
        "customagents",
        "Custom agents",
        &collapsed,
        custom_agents_body(&settings, &inputs, &handle, chrome),
        &handle,
        dimmed,
        border,
    ));

    // ── Layouts ──
    col = col.child(section(
        "layouts",
        "Layouts",
        &collapsed,
        layouts_body(&settings, &inputs, &handle, chrome),
        &handle,
        dimmed,
        border,
    ));

    // ── Editor ──
    let mut body = section_body();
    body = body.child(row(
        "Font family",
        "Editor font; press enter to apply.",
        ed.font_family != ed_defaults.font_family,
        None,
        div()
            .w(px(280.0))
            .flex_none()
            .child(inputs.font.clone())
            .into_any_element(),
        chrome,
    ));
    body = body.child(row(
        "Font size",
        "Editor font size in points.",
        ed.font_size != ed_defaults.font_size,
        None,
        stepper(
            "font-size",
            ed.font_size as i64,
            (8, 24),
            format!("{}", ed.font_size as i64),
            border,
            &handle,
            |root, v, cx| {
                let mut editor = root.settings.editor.clone();
                editor.font_size = v as f32;
                write_editor(root, editor, cx);
            },
        ),
        chrome,
    ));
    body = body.child(row(
        "Tab width",
        "Spaces per indentation level.",
        ed.tab_width != ed_defaults.tab_width,
        None,
        stepper(
            "tab-width",
            ed.tab_width as i64,
            (2, 8),
            ed.tab_width.to_string(),
            border,
            &handle,
            |root, v, cx| {
                let mut editor = root.settings.editor.clone();
                editor.tab_width = v as u32;
                write_editor(root, editor, cx);
            },
        ),
        chrome,
    ));
    {
        let h = handle.clone();
        let autosave = ed.autosave;
        body = body.child(row(
            "Autosave",
            "Save editor buffers automatically.",
            ed.autosave != ed_defaults.autosave,
            None,
            Switch::new("editor-autosave")
                .checked(autosave)
                .aria_label("Autosave editor buffers")
                .size(Size::Sm)
                .on_change(move |_, _, cx| {
                    h.update(cx, |root, cx| {
                        let mut editor = root.settings.editor.clone();
                        editor.autosave = !autosave;
                        write_editor(root, editor, cx);
                    });
                })
                .into_any_element(),
            chrome,
        ));
    }
    col = col.child(section(
        "editor", "Editor", &collapsed, body, &handle, dimmed, border,
    ));

    // ── Servers ──
    let mut body = section_body();
    body = body.child(
        Text::new(
            "Background HTTP servers this app can run: the mobile companion (browse \
             projects, tasks, and runs from your phone, and send follow-ups into a live \
             agent) and the agent control surface (lets a running agent orchestrate its \
             own fleet from inside its worktree).",
        )
        .size(Size::Xs)
        .dimmed(),
    );
    {
        let h = handle.clone();
        let enabled = settings.companion.enabled;
        let bind = settings.companion.bind.clone();
        body = body.child(row(
            "Enable companion",
            "Off by default. Refuses to start without a token, even on loopback — set \
             companion.token or the ASYLUM_COMPANION_TOKEN environment variable first.",
            enabled != defaults.companion.enabled,
            None,
            Switch::new("companion-enabled")
                .checked(enabled)
                .aria_label("Enable the mobile companion server")
                .size(Size::Sm)
                .on_change(move |_, _, cx| {
                    let bind = bind.clone();
                    h.update(cx, |root, cx| {
                        write_companion(root, !enabled, &bind, cx);
                    });
                })
                .into_any_element(),
            chrome,
        ));
    }
    body = body.child(row(
        "Companion bind address",
        "Loopback by default; set e.g. 0.0.0.0:8787 to reach it from a phone on the LAN. \
         Press enter to apply.",
        settings.companion.bind != defaults.companion.bind,
        None,
        div()
            .w(px(280.0))
            .flex_none()
            .child(inputs.companion_bind.clone())
            .into_any_element(),
        chrome,
    ));
    {
        let has_token = !settings.companion.token.trim().is_empty();
        let (label, color) = if has_token {
            ("token set", ColorName::Green)
        } else {
            ("token missing", ColorName::Red)
        };
        body = body.child(row(
            "Companion token",
            "Required whenever the server is enabled. Never shown here — set it via \
             companion.token in settings.json or the ASYLUM_COMPANION_TOKEN environment \
             variable.",
            false,
            None,
            Badge::new(label)
                .color(color)
                .variant(Variant::Light)
                .into_any_element(),
            chrome,
        ));
    }
    {
        let h = handle.clone();
        let enabled = settings.control.enabled;
        let bind = settings.control.bind.clone();
        body = body.child(row(
            "Enable control",
            "Lets a running agent list siblings, queue a helper run, or report its \
             activity. When control.token is left unset, a per-session token is \
             generated automatically and injected into each managed agent — nothing is \
             written back to settings.json.",
            enabled != defaults.control.enabled,
            None,
            Switch::new("control-enabled")
                .checked(enabled)
                .aria_label("Enable the agent control surface")
                .size(Size::Sm)
                .on_change(move |_, _, cx| {
                    let bind = bind.clone();
                    h.update(cx, |root, cx| {
                        write_control(root, !enabled, &bind, cx);
                    });
                })
                .into_any_element(),
            chrome,
        ));
    }
    body = body.child(row(
        "Control bind address",
        "Loopback only — a non-loopback bind is refused at startup. Press enter to apply.",
        settings.control.bind != defaults.control.bind,
        None,
        div()
            .w(px(280.0))
            .flex_none()
            .child(inputs.control_bind.clone())
            .into_any_element(),
        chrome,
    ));
    col = col.child(section(
        "servers", "Servers", &collapsed, body, &handle, dimmed, border,
    ));

    // ── Secrets keep ──
    col = col.child(section(
        "keep",
        "Secrets keep",
        &collapsed,
        keep_body(&inputs, &handle, chrome),
        &handle,
        dimmed,
        border,
    ));

    // ── Secrets proxy ──
    let mut body = section_body();
    body = body.child(
        Text::new(
            "Let agents call external APIs without seeing the keys. Define upstreams below \
             and store each key in the encrypted keep with `asylum keep set <name>`. \
             See docs/secrets.md.",
        )
        .size(Size::Xs)
        .dimmed(),
    );
    {
        let h = handle.clone();
        let enabled = settings.proxy.enabled;
        let bind = settings.proxy.bind.clone();
        body = body.child(row(
            "Enable",
            "Run the loopback secrets proxy; agents reach it via `asylum call`.",
            enabled != defaults.proxy.enabled,
            None,
            Switch::new("proxy-enabled")
                .checked(enabled)
                .aria_label("Enable the secrets proxy")
                .size(Size::Sm)
                .on_change(move |_, _, cx| {
                    let bind = bind.clone();
                    h.update(cx, |root, cx| {
                        write(
                            root,
                            "proxy",
                            json!({ "enabled": !enabled, "bind": bind }),
                            cx,
                        );
                    });
                })
                .into_any_element(),
            chrome,
        ));
    }
    body = body.child(row(
        "Bind address",
        "Loopback only — a non-loopback bind is refused. Press enter to apply.",
        settings.proxy.bind != defaults.proxy.bind,
        None,
        div()
            .w(px(280.0))
            .flex_none()
            .child(inputs.proxy_bind.clone())
            .into_any_element(),
        chrome,
    ));
    // Each upstream shows whether its keep secret is present; Add/Edit/Remove
    // write the whole `upstreams` list back through config::edit.
    body = body.child(upstreams_editor(&settings, &inputs, &handle, chrome));
    col = col.child(section(
        "proxy",
        "Secrets proxy",
        &collapsed,
        body,
        &handle,
        dimmed,
        border,
    ));

    // ── MCP gateway ──
    let mut body = section_body();
    body = body.child(
        Text::new(
            "One MCP server every agent connects to, fronting the servers below under \
             per-service namespaces (github__create_pr). Loopback-only and scoped per \
             run. Add and edit the aggregated servers below. See docs/mcp.md.",
        )
        .size(Size::Xs)
        .dimmed(),
    );
    {
        let h = handle.clone();
        let enabled = settings.mcp.enabled;
        let bind = settings.mcp.bind.clone();
        let expose = settings.mcp.expose.clone();
        body = body.child(row(
            "Enable",
            "Run the loopback MCP gateway; agents connect to one aggregated server.",
            enabled != defaults.mcp.enabled,
            None,
            Switch::new("mcp-enabled")
                .checked(enabled)
                .aria_label("Enable the MCP gateway")
                .size(Size::Sm)
                .on_change(move |_, _, cx| {
                    let bind = bind.clone();
                    let expose = expose.clone();
                    h.update(cx, |root, cx| {
                        write_mcp(root, !enabled, &bind, &expose, cx);
                    });
                })
                .into_any_element(),
            chrome,
        ));
    }
    body = body.child(row(
        "Bind address",
        "Loopback only — a non-loopback bind is refused. Agents reach it at /mcp. \
         Press enter to apply.",
        settings.mcp.bind != defaults.mcp.bind,
        None,
        div()
            .w(px(280.0))
            .flex_none()
            .child(inputs.mcp_bind.clone())
            .into_any_element(),
        chrome,
    ));
    body = body.child(row(
        "Tool exposure",
        "direct lists every tool; search advertises a find/call pair so tool \
         definitions load on demand (keeps a wide fleet's context small).",
        settings.mcp.expose != defaults.mcp.expose,
        None,
        expose_control(
            &settings.mcp.expose,
            settings.mcp.enabled,
            &settings.mcp.bind,
            &handle,
        ),
        chrome,
    ));
    body = body.child(mcp_servers_editor(&settings, &inputs, &handle, chrome));
    col = col.child(section(
        "mcp",
        "MCP gateway",
        &collapsed,
        body,
        &handle,
        dimmed,
        border,
    ));

    // ── Keybindings ──
    let mut body = section_body();
    body = body.child(
        Text::new(
            "Defaults layered with `keybindings` in settings.json — add \"chord=action\" \
             entries to rebind, or \"chord=\" to unbind.",
        )
        .size(Size::Xs)
        .dimmed(),
    );
    let keymap = config::Keymap::from_settings(&settings.keybindings);
    let mut keys = div().flex().flex_col().pt(px(6.0));
    for (chord, action) in keymap.bindings() {
        keys = keys.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .py(px(6.0))
                .border_b_1()
                .border_color(border)
                .child(Text::new(SharedString::from(action.to_string())).size(Size::Sm))
                .child(Kbd::new(SharedString::from(chord.to_string()))),
        );
    }
    body = body.child(keys);
    col = col.child(section(
        "keys",
        "Keybindings",
        &collapsed,
        body,
        &handle,
        dimmed,
        border,
    ));

    div()
        .id("settings-scroll")
        .size_full()
        .overflow_y_scroll()
        .child(col)
}

/// The theme values a row draws itself with, read once rather than per row.
#[derive(Clone, Copy)]
struct Chrome {
    border: gpui::Hsla,
    dimmed: gpui::Hsla,
    /// Font size for a row's description line.
    desc: gpui::Pixels,
}

/// The Settings accordion section keys, in display order. Stable ids for
/// collapse state.
pub const SECTIONS: [&str; 10] = [
    "general",
    "agents",
    "customagents",
    "layouts",
    "editor",
    "servers",
    "keep",
    "proxy",
    "mcp",
    "keys",
];

/// The collapse state a fresh Settings surface opens with: every section except
/// the first is collapsed, so the page opens compact.
pub fn default_collapsed() -> std::collections::HashSet<&'static str> {
    SECTIONS.iter().copied().skip(1).collect()
}

/// The collapse state that deep-links straight into one section (e.g. the
/// onboarding "Configure agents" button): every other section collapsed, so
/// the target section is the only one open when Settings first renders.
pub fn collapsed_except(section: &'static str) -> std::collections::HashSet<&'static str> {
    SECTIONS.iter().copied().filter(|s| *s != section).collect()
}

/// A section body: a column the section's rows are appended to.
fn section_body() -> gpui::Div {
    div().flex().flex_col().gap_1()
}

/// One collapsible accordion group. The header (a disclosure chevron + label
/// over the underline divider) is clickable and toggles the section's collapsed
/// state on [`Root`]; the body renders only when the section is expanded.
#[allow(clippy::too_many_arguments)]
fn section(
    key: &'static str,
    title: &'static str,
    collapsed: &std::collections::HashSet<&'static str>,
    body: gpui::Div,
    handle: &Entity<Root>,
    dimmed: gpui::Hsla,
    border: gpui::Hsla,
) -> impl IntoElement {
    let is_collapsed = collapsed.contains(key);
    let toggle = handle.clone();
    let chevron = if is_collapsed {
        "chevron-right"
    } else {
        "chevron-down"
    };

    let header = div()
        .id(SharedString::from(format!("settings-section-{key}")))
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .pt(px(22.0))
        .pb(px(8.0))
        .cursor_pointer()
        .child(crate::icons::icon(chevron, 14.0).text_color(dimmed))
        .child(
            div()
                .text_size(px(12.5))
                .text_color(dimmed)
                .child(SharedString::from(title)),
        )
        .on_click(move |_, _, cx| {
            toggle.update(cx, |root, cx| {
                // Toggle: remove if present, else insert.
                if !root.settings_collapsed.remove(key) {
                    root.settings_collapsed.insert(key);
                }
                cx.notify();
            });
        });

    div()
        .flex()
        .flex_col()
        .child(header)
        .child(div().w_full().h(px(1.0)).bg(border))
        .when(!is_collapsed, |d| d.child(div().pt(px(8.0)).child(body)))
}

/// One settings row: label + description on the left, the control (and a
/// Reset when the value differs from the default) on the right.
fn row(
    label: impl Into<SharedString>,
    desc: impl Into<SharedString>,
    modified: bool,
    reset: Option<gpui::AnyElement>,
    control: gpui::AnyElement,
    chrome: Chrome,
) -> impl IntoElement {
    // The description is a child of a *column* on purpose. gpui reports a text
    // element's min-content width as its whole single line, so as a row's flex
    // item it could never shrink and would slide under the control; as a
    // column's item it takes the column's width and wraps.
    let description = div()
        .flex()
        .flex_col()
        .min_w(px(0.0))
        .text_size(chrome.desc)
        .text_color(chrome.dimmed)
        .child(desc.into());

    let left = div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        // Take the space the control leaves and wrap the description inside it,
        // so every row's control lands on the same right edge.
        .flex_1()
        .min_w(px(0.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(Text::new(label.into()).size(Size::Sm))
                .when(modified, |d| {
                    d.child(
                        Badge::new("modified")
                            .color(ColorName::Blue)
                            .variant(Variant::Light),
                    )
                }),
        )
        .child(description);

    let mut right = div().flex().flex_row().items_center().gap_2().flex_none();
    if let Some(reset) = reset.filter(|_| modified) {
        right = right.child(reset);
    }
    right = right.child(control);

    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_3()
        .py(px(10.0))
        .border_b_1()
        .border_color(chrome.border)
        .child(left)
        .child(right)
}

/// A "Test" button that runs the agent's configured executable. It reports the
/// saved program, so press enter in the path field before testing an edit.
fn test_btn(agent: &agent::registry::Agent, handle: &Entity<Root>) -> gpui::AnyElement {
    let h = handle.clone();
    let id = agent.id.clone();
    let program = agent.program.clone();
    Button::new(SharedString::from(format!("test-{}", agent.id)), "Test")
        .variant(Variant::Default)
        .size(Size::Xs)
        .on_click(move |_, _, cx| {
            let (id, program) = (id.clone(), program.clone());
            h.update(cx, |root, cx| test_agent(root, id, program, cx));
        })
        .into_any_element()
}

/// What the last probe said, beside the agent's row.
fn test_result(test: Option<&Test>) -> gpui::AnyElement {
    let (label, color) = match test {
        None => return div().into_any_element(),
        Some(Test::Running) => ("testing…".to_string(), ColorName::Gray),
        Some(Test::Done(probe)) => {
            let color = if probe.ok() {
                ColorName::Green
            } else {
                ColorName::Red
            };
            let mark = if probe.ok() { '\u{2713}' } else { '\u{2717}' };
            (format!("{mark} {}", trunc(probe.message(), 28)), color)
        }
    };
    Badge::new(SharedString::from(label))
        .color(color)
        .variant(Variant::Light)
        .into_any_element()
}

/// Clip `text` to `max` characters so a long version banner or error can't
/// crowd the row's other controls.
fn trunc(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    text.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
}

/// A "Reset" button that removes `key` from settings.json.
fn reset_btn(id: &'static str, key: &'static str, handle: &Entity<Root>) -> gpui::AnyElement {
    let h = handle.clone();
    Button::new(id, "Reset")
        .variant(Variant::Subtle)
        .size(Size::Xs)
        .on_click(move |_, _, cx| {
            h.update(cx, |root, cx| reset(root, key, cx));
        })
        .into_any_element()
}

/// The theme picker: one chip per scheme, the active one filled.
fn theme_control(current: &str, handle: &Entity<Root>) -> gpui::AnyElement {
    let mut group = div().flex().flex_row().gap_1();
    for name in ["dark", "light"] {
        let active = current == name;
        let h = handle.clone();
        group = group.child(
            Button::new(SharedString::from(format!("theme-{name}")), name)
                .size(Size::Xs)
                .variant(if active {
                    Variant::Filled
                } else {
                    Variant::Default
                })
                .on_click(move |_, _, cx| {
                    h.update(cx, |root, cx| write(root, "theme", json!(name), cx));
                }),
        );
    }
    group.into_any_element()
}

/// The MCP tool-exposure picker: `direct` lists every tool, `search` advertises
/// the lazy find/call pair. Writes the `mcp` object, preserving enabled + bind.
fn expose_control(
    current: &str,
    enabled: bool,
    bind: &str,
    handle: &Entity<Root>,
) -> gpui::AnyElement {
    let mut group = div().flex().flex_row().gap_1();
    for mode in ["direct", "search"] {
        let active = current.eq_ignore_ascii_case(mode);
        let h = handle.clone();
        let bind = bind.to_string();
        group = group.child(
            Button::new(SharedString::from(format!("mcp-expose-{mode}")), mode)
                .size(Size::Xs)
                .variant(if active {
                    Variant::Filled
                } else {
                    Variant::Default
                })
                .on_click(move |_, _, cx| {
                    let bind = bind.clone();
                    h.update(cx, |root, cx| write_mcp(root, enabled, &bind, mode, cx));
                }),
        );
    }
    group.into_any_element()
}

/// One MCP server's row detail, status label, and status color: its transport +
/// target + scope, and whether it is enabled (and, for an authenticated HTTP
/// server, whether its secret is present in the keep).
fn mcp_server_status(server: &config::McpServer) -> (String, &'static str, ColorName) {
    let scope = if server.project == 0 {
        "global".to_string()
    } else {
        format!("project {}", server.project)
    };
    let target = if server.transport == "http" {
        server.url.clone()
    } else if server.args.is_empty() {
        server.command.clone()
    } else {
        format!("{} {}", server.command, server.args.join(" "))
    };
    let transport = if server.transport.is_empty() {
        "stdio"
    } else {
        server.transport.as_str()
    };
    let detail = format!("{transport} · {target} · {scope}");
    let (status, color) = if !server.enabled {
        ("disabled", ColorName::Gray)
    } else if server.transport == "http" && !server.secret.is_empty() {
        if crate::secrets::has_secret(&server.secret, server.project) {
            ("secret set", ColorName::Green)
        } else {
            ("secret missing", ColorName::Red)
        }
    } else {
        ("enabled", ColorName::Green)
    };
    (detail, status, color)
}

/// A −/value/+ stepper; each press clamps to `range` and runs `on_set`.
fn stepper(
    id: &'static str,
    value: i64,
    range: (i64, i64),
    display: String,
    border: gpui::Hsla,
    handle: &Entity<Root>,
    on_set: impl Fn(&mut Root, i64, &mut Context<Root>) + Clone + 'static,
) -> gpui::AnyElement {
    let button = |delta: i64, glyph: &'static str| {
        let h = handle.clone();
        let set = on_set.clone();
        let next = (value + delta).clamp(range.0, range.1);
        let label = if delta < 0 {
            "Decrease value"
        } else {
            "Increase value"
        };
        div()
            .id(SharedString::from(format!("{id}-{glyph}")))
            .px(px(6.0))
            .cursor_pointer()
            .tab_index(0)
            .role(gpui::accesskit::Role::Button)
            .aria_label(label)
            .focus_visible(move |style| style.border_1().border_color(border))
            .child(SharedString::from(glyph))
            .on_click(move |_, _, cx| {
                if next == value {
                    return;
                }
                h.update(cx, |root, cx| set(root, next, cx));
            })
    };
    div()
        .flex()
        .flex_row()
        .items_center()
        .px(px(4.0))
        .py(px(2.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(border)
        .child(button(-1, "−"))
        .child(
            div()
                .min_w(px(56.0))
                .flex()
                .justify_center()
                .text_size(px(12.5))
                .child(SharedString::from(display)),
        )
        .child(button(1, "+"))
        .into_any_element()
}

// ── List editors: MCP servers, proxy upstreams, custom agents, layouts ──────
//
// Each list is edited as a whole and written back through `config::edit`, so
// the user's comments and every other key survive. The pure builders below
// turn the form's raw text into a validated model value (or an error message
// shown inline); the render + write plumbing follows. Names that must be
// addressable slugs are validated before any write.

/// Split a whitespace-separated field into tokens (a command's args).
fn split_ws(raw: &str) -> Vec<String> {
    raw.split_whitespace().map(str::to_string).collect()
}

/// Split a comma- or whitespace-separated field into a list (allow/deny tool
/// names, a layout's agent ids).
pub fn split_list(raw: &str) -> Vec<String> {
    raw.split([',', ' ', '\t', '\n', '\r'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Whether `name` is a valid addressable slug (lowercase `[a-z0-9-]`, no
/// leading/trailing `-`). Reuses the MCP gateway's own rule, so a server name
/// that saves here is one the gateway will actually accept; proxy upstream
/// names (a URL path segment) follow the same rule.
pub fn valid_slug(name: &str) -> bool {
    mcp::namespace::is_valid_service(name)
}

/// Validate a keep secret name: non-empty and space-free (it is a map key and a
/// `{secret:NAME}` reference). Returns the trimmed name.
pub fn valid_secret_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("A secret name is required.".into());
    }
    if name.contains(char::is_whitespace) {
        return Err("A secret name cannot contain spaces.".into());
    }
    Ok(name.to_string())
}

/// Build an `McpServer` from the editor's raw fields, preserving `base`'s
/// untouched fields (env, header, format). `Err` carries an inline message; the
/// list is not written until this is `Ok`.
#[allow(clippy::too_many_arguments)]
pub fn build_server(
    base: &config::McpServer,
    name: &str,
    transport: &str,
    command: &str,
    args: &str,
    url: &str,
    secret: &str,
    allow: &str,
    deny: &str,
    project: i64,
    enabled: bool,
) -> Result<config::McpServer, String> {
    let name = name.trim();
    if !valid_slug(name) {
        return Err("Name must be a lowercase slug ([a-z0-9-], no leading/trailing '-').".into());
    }
    let transport = if transport == "http" { "http" } else { "stdio" };
    let command = command.trim();
    let url = url.trim();
    if transport == "stdio" && command.is_empty() {
        return Err("A stdio server needs a command.".into());
    }
    if transport == "http" && !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("An http server needs a URL starting with http:// or https://.".into());
    }
    Ok(config::McpServer {
        name: name.to_string(),
        transport: transport.to_string(),
        command: command.to_string(),
        args: split_ws(args),
        url: url.to_string(),
        secret: secret.trim().to_string(),
        allow: split_list(allow),
        deny: split_list(deny),
        project,
        enabled,
        env: base.env.clone(),
        header: base.header.clone(),
        format: base.format.clone(),
    })
}

/// Build an `Upstream` from the editor's fields, preserving `base`'s header and
/// format (not exposed by the form). The secret is a *name*, never a value.
pub fn build_upstream(
    base: &config::Upstream,
    name: &str,
    base_url: &str,
    secret: &str,
    project: i64,
) -> Result<config::Upstream, String> {
    let name = name.trim();
    if !valid_slug(name) {
        return Err("Name must be a lowercase slug ([a-z0-9-], no leading/trailing '-').".into());
    }
    let base_url = base_url.trim();
    if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
        return Err("Base URL must start with http:// or https://.".into());
    }
    let secret = secret.trim();
    if secret.is_empty() {
        return Err("A secret name is required (the keep entry to inject).".into());
    }
    Ok(config::Upstream {
        name: name.to_string(),
        base_url: base_url.to_string(),
        secret: secret.to_string(),
        header: base.header.clone(),
        format: base.format.clone(),
        project,
    })
}

/// Build a `CustomAgent`, refusing an empty id/program (matching `config`'s
/// load-time validation). An empty name falls back to the id.
pub fn build_custom_agent(
    id: &str,
    name: &str,
    icon: &str,
    program: &str,
    args: &str,
    delivery: &str,
) -> Result<config::CustomAgent, String> {
    let id = id.trim();
    if id.is_empty() {
        return Err("An id is required.".into());
    }
    if id.contains(char::is_whitespace) {
        return Err("The id cannot contain spaces (it names branches and runs).".into());
    }
    let program = program.trim();
    if program.is_empty() {
        return Err("A program is required.".into());
    }
    let name = name.trim();
    let delivery = if delivery == "stdin" { "stdin" } else { "arg" };
    Ok(config::CustomAgent {
        id: id.to_string(),
        name: if name.is_empty() {
            id.to_string()
        } else {
            name.to_string()
        },
        icon: icon
            .trim()
            .chars()
            .next()
            .map(String::from)
            .unwrap_or_default(),
        program: program.to_string(),
        args: split_ws(args),
        delivery: delivery.to_string(),
    })
}

/// Build a `Layout`, refusing an empty name or an empty agent list.
pub fn build_layout(
    name: &str,
    description: &str,
    agents: &str,
    concurrency: u32,
) -> Result<config::Layout, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("A name is required.".into());
    }
    let agents = split_list(agents);
    if agents.is_empty() {
        return Err("List at least one agent id.".into());
    }
    Ok(config::Layout {
        name: name.to_string(),
        description: description.trim().to_string(),
        agents,
        concurrency,
    })
}

/// The scope options a form offers: always Global, the current project when one
/// is focused, and (when editing) the item's own scope if it is elsewhere.
/// Values are the project id as a string (`"0"` = global).
pub fn scope_options(current_project: Option<i64>, selected: i64) -> Vec<(String, String)> {
    let mut opts = vec![("0".to_string(), "Global".to_string())];
    if let Some(cur) = current_project {
        opts.push((cur.to_string(), "This project".to_string()));
    }
    if selected != 0 && current_project != Some(selected) {
        opts.push((selected.to_string(), format!("Project {selected}")));
    }
    opts
}

/// Human label for a keep scope key (`global` / `project:<id>`).
pub fn scope_display(scope_key: &str) -> String {
    match scope_key.strip_prefix("project:") {
        Some(id) => format!("Project {id} keep"),
        None => "Global keep".to_string(),
    }
}

/// The project id a keep scope key names (`global` → 0).
pub fn scope_key_project(scope_key: &str) -> i64 {
    scope_key
        .strip_prefix("project:")
        .and_then(|id| id.parse().ok())
        .unwrap_or(0)
}

// ── Writing a whole list back (or dropping the key at its default) ───────────

fn write_servers(root: &mut Root, servers: Vec<config::McpServer>, cx: &mut Context<Root>) {
    if servers.is_empty() {
        reset(root, "mcp_servers", cx);
    } else {
        write(root, "mcp_servers", json!(servers), cx);
    }
}

fn write_upstreams(root: &mut Root, upstreams: Vec<config::Upstream>, cx: &mut Context<Root>) {
    if upstreams.is_empty() {
        reset(root, "upstreams", cx);
    } else {
        write(root, "upstreams", json!(upstreams), cx);
    }
}

fn write_custom_agents(root: &mut Root, agents: Vec<config::CustomAgent>, cx: &mut Context<Root>) {
    if agents.is_empty() {
        reset(root, "custom_agents", cx);
    } else {
        write(root, "custom_agents", json!(agents), cx);
    }
}

fn write_layouts(root: &mut Root, layouts: Vec<config::Layout>, cx: &mut Context<Root>) {
    // The default is the built-in presets, not an empty list: restoring exactly
    // those drops the key, but an intentionally empty list is written as-is.
    if layouts == config::Layout::builtins() {
        reset(root, "layouts", cx);
    } else {
        write(root, "layouts", json!(layouts), cx);
    }
}

// ── Opening an add/edit form (seeded from the row, or blank for a new one) ───

fn open_server_form(root: &mut Root, index: Option<usize>, cx: &mut Context<Root>) {
    let base = index
        .and_then(|i| root.settings.mcp_servers.get(i).cloned())
        .unwrap_or_default();
    let transport = if base.transport == "http" {
        "http"
    } else {
        "stdio"
    }
    .to_string();
    let form = ServerForm {
        index,
        transport,
        project: base.project,
        current_project: root.project_id,
        enabled: base.enabled,
        name: field(cx, &base.name, "github", false),
        command: field(cx, &base.command, "npx", false),
        args: field(
            cx,
            &base.args.join(" "),
            "-y @modelcontextprotocol/server-github",
            false,
        ),
        url: field(cx, &base.url, "https://mcp.example.com/mcp", false),
        secret: field(cx, &base.secret, "keep secret name (optional)", false),
        allow: field(
            cx,
            &base.allow.join(" "),
            "only these tools (optional)",
            false,
        ),
        deny: field(
            cx,
            &base.deny.join(" "),
            "hide these tools (optional)",
            false,
        ),
        base,
        error: None,
    };
    if let Some(inputs) = root.settings_inputs.as_mut() {
        inputs.server_form = Some(form);
    }
    cx.notify();
}

fn open_upstream_form(root: &mut Root, index: Option<usize>, cx: &mut Context<Root>) {
    let base = index
        .and_then(|i| root.settings.upstreams.get(i).cloned())
        .unwrap_or_default();
    let form = UpstreamForm {
        index,
        project: base.project,
        current_project: root.project_id,
        name: field(cx, &base.name, "openai", false),
        base_url: field(cx, &base.base_url, "https://api.openai.com", false),
        secret: field(cx, &base.secret, "keep secret name", false),
        base,
        error: None,
    };
    if let Some(inputs) = root.settings_inputs.as_mut() {
        inputs.upstream_form = Some(form);
    }
    cx.notify();
}

fn open_agent_form(root: &mut Root, index: Option<usize>, cx: &mut Context<Root>) {
    let base = index
        .and_then(|i| root.settings.custom_agents.get(i).cloned())
        .unwrap_or_default();
    let delivery = if base.delivery == "stdin" {
        "stdin"
    } else {
        "arg"
    }
    .to_string();
    let form = CustomAgentForm {
        index,
        delivery,
        id: field(cx, &base.id, "my-agent", false),
        name: field(cx, &base.name, "My Agent", false),
        icon: field(cx, &base.icon, "🤖", false),
        program: field(cx, &base.program, "my-agent-cli", false),
        args: field(cx, &base.args.join(" "), "--prompt {prompt}", false),
        error: None,
    };
    if let Some(inputs) = root.settings_inputs.as_mut() {
        inputs.agent_form = Some(form);
    }
    cx.notify();
}

fn open_layout_form(root: &mut Root, index: Option<usize>, cx: &mut Context<Root>) {
    let base = index
        .and_then(|i| root.settings.layouts.get(i).cloned())
        .unwrap_or_default();
    let form = LayoutForm {
        index,
        concurrency: base.concurrency,
        name: field(cx, &base.name, "duel", false),
        description: field(cx, &base.description, "Two agents, head to head", false),
        agents: field(cx, &base.agents.join(", "), "claude-code, codex", false),
        error: None,
    };
    if let Some(inputs) = root.settings_inputs.as_mut() {
        inputs.layout_form = Some(form);
    }
    cx.notify();
}

fn open_secret_form(root: &mut Root, cx: &mut Context<Root>) {
    let form = SecretForm {
        project: 0,
        current_project: root.project_id,
        name: field(cx, "", "OPENAI_API_KEY", false),
        value: field(cx, "", "secret value", true),
        error: None,
    };
    if let Some(inputs) = root.settings_inputs.as_mut() {
        inputs.secret_form = Some(form);
    }
    cx.notify();
}

// ── Saving a form: read every field, build, upsert into the list, write ──────

/// Record an error on the still-open MCP-server form.
fn server_error(root: &mut Root, message: String, cx: &mut Context<Root>) {
    if let Some(f) = root
        .settings_inputs
        .as_mut()
        .and_then(|i| i.server_form.as_mut())
    {
        f.error = Some(SharedString::from(message));
    }
    cx.notify();
}

fn save_server(root: &mut Root, cx: &mut Context<Root>) {
    let Some(form) = root
        .settings_inputs
        .as_ref()
        .and_then(|i| i.server_form.clone())
    else {
        return;
    };
    let server = build_server(
        &form.base,
        &form.name.read(cx).text(),
        &form.transport,
        &form.command.read(cx).text(),
        &form.args.read(cx).text(),
        &form.url.read(cx).text(),
        &form.secret.read(cx).text(),
        &form.allow.read(cx).text(),
        &form.deny.read(cx).text(),
        form.project,
        form.enabled,
    );
    match server {
        Ok(server) => {
            let mut servers = root.settings.mcp_servers.clone();
            match form.index {
                Some(i) if i < servers.len() => servers[i] = server,
                _ => servers.push(server),
            }
            if let Some(inputs) = root.settings_inputs.as_mut() {
                inputs.server_form = None;
            }
            write_servers(root, servers, cx);
        }
        Err(e) => server_error(root, e, cx),
    }
}

fn save_upstream(root: &mut Root, cx: &mut Context<Root>) {
    let Some(form) = root
        .settings_inputs
        .as_ref()
        .and_then(|i| i.upstream_form.clone())
    else {
        return;
    };
    let up = build_upstream(
        &form.base,
        &form.name.read(cx).text(),
        &form.base_url.read(cx).text(),
        &form.secret.read(cx).text(),
        form.project,
    );
    match up {
        Ok(up) => {
            let mut upstreams = root.settings.upstreams.clone();
            match form.index {
                Some(i) if i < upstreams.len() => upstreams[i] = up,
                _ => upstreams.push(up),
            }
            if let Some(inputs) = root.settings_inputs.as_mut() {
                inputs.upstream_form = None;
            }
            write_upstreams(root, upstreams, cx);
        }
        Err(e) => {
            if let Some(f) = root
                .settings_inputs
                .as_mut()
                .and_then(|i| i.upstream_form.as_mut())
            {
                f.error = Some(SharedString::from(e));
            }
            cx.notify();
        }
    }
}

fn save_agent(root: &mut Root, cx: &mut Context<Root>) {
    let Some(form) = root
        .settings_inputs
        .as_ref()
        .and_then(|i| i.agent_form.clone())
    else {
        return;
    };
    let agent = build_custom_agent(
        &form.id.read(cx).text(),
        &form.name.read(cx).text(),
        &form.icon.read(cx).text(),
        &form.program.read(cx).text(),
        &form.args.read(cx).text(),
        &form.delivery,
    );
    match agent {
        Ok(agent) => {
            let mut agents = root.settings.custom_agents.clone();
            match form.index {
                Some(i) if i < agents.len() => agents[i] = agent,
                _ => agents.push(agent),
            }
            if let Some(inputs) = root.settings_inputs.as_mut() {
                inputs.agent_form = None;
            }
            write_custom_agents(root, agents, cx);
        }
        Err(e) => {
            if let Some(f) = root
                .settings_inputs
                .as_mut()
                .and_then(|i| i.agent_form.as_mut())
            {
                f.error = Some(SharedString::from(e));
            }
            cx.notify();
        }
    }
}

fn save_layout(root: &mut Root, cx: &mut Context<Root>) {
    let Some(form) = root
        .settings_inputs
        .as_ref()
        .and_then(|i| i.layout_form.clone())
    else {
        return;
    };
    let layout = build_layout(
        &form.name.read(cx).text(),
        &form.description.read(cx).text(),
        &form.agents.read(cx).text(),
        form.concurrency,
    );
    match layout {
        Ok(layout) => {
            let mut layouts = root.settings.layouts.clone();
            match form.index {
                Some(i) if i < layouts.len() => layouts[i] = layout,
                _ => layouts.push(layout),
            }
            if let Some(inputs) = root.settings_inputs.as_mut() {
                inputs.layout_form = None;
            }
            write_layouts(root, layouts, cx);
        }
        Err(e) => {
            if let Some(f) = root
                .settings_inputs
                .as_mut()
                .and_then(|i| i.layout_form.as_mut())
            {
                f.error = Some(SharedString::from(e));
            }
            cx.notify();
        }
    }
}

fn save_secret(root: &mut Root, cx: &mut Context<Root>) {
    let Some(form) = root
        .settings_inputs
        .as_ref()
        .and_then(|i| i.secret_form.clone())
    else {
        return;
    };
    let name = match valid_secret_name(&form.name.read(cx).text()) {
        Ok(name) => name,
        Err(e) => return secret_error(root, e, cx),
    };
    let value = form.value.read(cx).text();
    if value.is_empty() {
        return secret_error(root, "A value is required.".into(), cx);
    }
    match crate::secrets::keep_set(form.project, &name, &value) {
        Ok(()) => {
            if let Some(inputs) = root.settings_inputs.as_mut() {
                inputs.secret_form = None;
            }
            cx.notify();
        }
        Err(e) => secret_error(root, e, cx),
    }
}

fn secret_error(root: &mut Root, message: String, cx: &mut Context<Root>) {
    if let Some(f) = root
        .settings_inputs
        .as_mut()
        .and_then(|i| i.secret_form.as_mut())
    {
        f.error = Some(SharedString::from(message));
    }
    cx.notify();
}

/// Read the passphrase, unlock (or create) the keep, then clear the field. The
/// passphrase is never stored; only the unlocked keep (in memory) survives.
fn unlock_keep_action(root: &mut Root, cx: &mut Context<Root>) {
    let Some(field) = root.settings_inputs.as_ref().map(|i| i.keep_pass.clone()) else {
        return;
    };
    let pass = field.read(cx).text();
    match crate::secrets::unlock_keep(&pass) {
        Ok(()) => {
            field.update(cx, |i, cx| i.set_text("", cx));
            if let Some(inputs) = root.settings_inputs.as_mut() {
                inputs.keep_error = None;
            }
        }
        Err(e) => {
            if let Some(inputs) = root.settings_inputs.as_mut() {
                inputs.keep_error = Some(SharedString::from(e));
            }
        }
    }
    cx.notify();
}

// ── Rendering the editors ────────────────────────────────────────────────────

/// A bordered card a form's fields sit in.
fn form_frame(border: gpui::Hsla) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .mt(px(8.0))
        .p(px(12.0))
        .rounded(px(8.0))
        .border_1()
        .border_color(border)
}

/// A form field with a small caption above it.
fn labeled(label: &str, input: &Entity<guise::TextInput>, dimmed: gpui::Hsla) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(3.0))
        .child(
            div()
                .text_size(px(11.5))
                .text_color(dimmed)
                .child(SharedString::from(label.to_string())),
        )
        .child(input.clone())
}

/// The inline error badge for a form, or an empty node when there is none.
fn form_error(err: &Option<SharedString>) -> gpui::AnyElement {
    match err {
        Some(e) => Badge::new(e.clone())
            .color(ColorName::Red)
            .variant(Variant::Light)
            .into_any_element(),
        None => div().into_any_element(),
    }
}

/// A small button whose click runs `on_click` against the root.
fn mut_btn(
    id: String,
    label: &'static str,
    variant: Variant,
    handle: &Entity<Root>,
    on_click: impl Fn(&mut Root, &mut Context<Root>) + 'static,
) -> Button {
    let h = handle.clone();
    Button::new(SharedString::from(id), label)
        .variant(variant)
        .size(Size::Xs)
        .on_click(move |_, _, cx| {
            h.update(cx, |root, cx| on_click(root, cx));
        })
}

/// A Save/Cancel pair for a form.
fn save_cancel(
    prefix: &str,
    handle: &Entity<Root>,
    save: impl Fn(&mut Root, &mut Context<Root>) + 'static,
    cancel: impl Fn(&mut Root) + 'static,
) -> gpui::Div {
    let hs = handle.clone();
    let hc = handle.clone();
    div()
        .flex()
        .flex_row()
        .gap_2()
        .pt(px(4.0))
        .child(
            Button::new(SharedString::from(format!("{prefix}-save")), "Save")
                .size(Size::Xs)
                .variant(Variant::Filled)
                .on_click(move |_, _, cx| {
                    hs.update(cx, |root, cx| save(root, cx));
                }),
        )
        .child(
            Button::new(SharedString::from(format!("{prefix}-cancel")), "Cancel")
                .size(Size::Xs)
                .variant(Variant::Subtle)
                .on_click(move |_, _, cx| {
                    hc.update(cx, |root, cx| {
                        cancel(root);
                        cx.notify();
                    });
                }),
        )
}

/// A single-select chip group; picking one runs `on_pick` with its value.
fn chips(
    id_prefix: &str,
    options: Vec<(String, String)>,
    current: &str,
    handle: &Entity<Root>,
    on_pick: impl Fn(&mut Root, String, &mut Context<Root>) + Clone + 'static,
) -> gpui::AnyElement {
    let mut group = div().flex().flex_row().gap_1();
    for (value, label) in options {
        let active = current == value.as_str();
        let h = handle.clone();
        let pick = on_pick.clone();
        group = group.child(
            Button::new(SharedString::from(format!("{id_prefix}-{value}")), label)
                .size(Size::Xs)
                .variant(if active {
                    Variant::Filled
                } else {
                    Variant::Default
                })
                .on_click(move |_, _, cx| {
                    let value = value.clone();
                    let pick = pick.clone();
                    h.update(cx, |root, cx| {
                        pick(root, value, cx);
                        cx.notify();
                    });
                }),
        );
    }
    group.into_any_element()
}

/// A scope picker (Global / This project / an item's own scope), reporting the
/// chosen project id (`0` = global).
fn scope_chips(
    id_prefix: &str,
    current_project: Option<i64>,
    selected: i64,
    handle: &Entity<Root>,
    on_pick: impl Fn(&mut Root, i64, &mut Context<Root>) + Clone + 'static,
) -> gpui::AnyElement {
    let options = scope_options(current_project, selected);
    chips(
        id_prefix,
        options,
        &selected.to_string(),
        handle,
        move |root, v, cx| {
            on_pick(root, v.parse().unwrap_or(0), cx);
        },
    )
}

/// The MCP-server list with per-row Edit/Remove, an Add button, and the open
/// editor form.
fn mcp_servers_editor(
    settings: &config::Settings,
    inputs: &Inputs,
    handle: &Entity<Root>,
    chrome: Chrome,
) -> gpui::Div {
    let mut col = div().flex().flex_col().gap_1();
    if settings.mcp_servers.is_empty() && inputs.server_form.is_none() {
        col = col.child(
            Text::new(
                "No MCP servers yet. Add a stdio command or an http url; each is exposed to \
                 agents under its own namespace.",
            )
            .size(Size::Xs)
            .dimmed(),
        );
    }
    for (i, server) in settings.mcp_servers.iter().enumerate() {
        let (detail, status, color) = mcp_server_status(server);
        let controls = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(Badge::new(status).color(color).variant(Variant::Light))
            .child(mut_btn(
                format!("mcp-edit-{i}"),
                "Edit",
                Variant::Subtle,
                handle,
                move |root, cx| open_server_form(root, Some(i), cx),
            ))
            .child(mut_btn(
                format!("mcp-remove-{i}"),
                "Remove",
                Variant::Subtle,
                handle,
                move |root, cx| {
                    let mut v = root.settings.mcp_servers.clone();
                    if i < v.len() {
                        v.remove(i);
                    }
                    write_servers(root, v, cx);
                },
            ));
        col = col.child(row(
            SharedString::from(server.name.clone()),
            SharedString::from(detail),
            false,
            None,
            controls.into_any_element(),
            chrome,
        ));
    }
    match &inputs.server_form {
        Some(form) => col.child(server_form_card(form, handle, chrome)),
        None => col.child(div().pt(px(6.0)).child(mut_btn(
            "mcp-add".to_string(),
            "Add server",
            Variant::Default,
            handle,
            |root, cx| open_server_form(root, None, cx),
        ))),
    }
}

fn server_form_card(form: &ServerForm, handle: &Entity<Root>, chrome: Chrome) -> gpui::AnyElement {
    let dimmed = chrome.dimmed;
    let title = if form.index.is_some() {
        "Edit server"
    } else {
        "New server"
    };
    let enabled = form.enabled;
    let h = handle.clone();
    let mut card = form_frame(chrome.border)
        .child(Text::new(title).size(Size::Sm))
        .child(chips(
            "mcp-transport",
            vec![
                ("stdio".to_string(), "stdio".to_string()),
                ("http".to_string(), "http".to_string()),
            ],
            &form.transport,
            handle,
            |root, v, _cx| {
                if let Some(f) = root
                    .settings_inputs
                    .as_mut()
                    .and_then(|i| i.server_form.as_mut())
                {
                    f.transport = v;
                }
            },
        ))
        .child(labeled("Name (namespace)", &form.name, dimmed));
    if form.transport == "http" {
        card = card.child(labeled("URL", &form.url, dimmed)).child(labeled(
            "Secret name (optional)",
            &form.secret,
            dimmed,
        ));
    } else {
        card = card
            .child(labeled("Command", &form.command, dimmed))
            .child(labeled("Args", &form.args, dimmed));
    }
    card = card
        .child(labeled("Allow tools (optional)", &form.allow, dimmed))
        .child(labeled("Deny tools (optional)", &form.deny, dimmed))
        .child(scope_chips(
            "mcp-scope",
            form.current_project,
            form.project,
            handle,
            |root, project, _cx| {
                if let Some(f) = root
                    .settings_inputs
                    .as_mut()
                    .and_then(|i| i.server_form.as_mut())
                {
                    f.project = project;
                }
            },
        ))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(Text::new("Enabled").size(Size::Xs).dimmed())
                .child(
                    Switch::new("mcp-form-enabled")
                        .checked(enabled)
                        .size(Size::Sm)
                        .aria_label("Enable this server")
                        .on_change(move |_, _, cx| {
                            h.update(cx, |root, cx| {
                                if let Some(f) = root
                                    .settings_inputs
                                    .as_mut()
                                    .and_then(|i| i.server_form.as_mut())
                                {
                                    f.enabled = !enabled;
                                }
                                cx.notify();
                            });
                        }),
                ),
        );
    if form.error.is_some() {
        card = card.child(form_error(&form.error));
    }
    card.child(save_cancel("mcp", handle, save_server, |root| {
        if let Some(inputs) = root.settings_inputs.as_mut() {
            inputs.server_form = None;
        }
    }))
    .into_any_element()
}

/// The proxy-upstream list with per-row Edit/Remove, an Add button, and the
/// open editor form.
fn upstreams_editor(
    settings: &config::Settings,
    inputs: &Inputs,
    handle: &Entity<Root>,
    chrome: Chrome,
) -> gpui::Div {
    let mut col = div().flex().flex_col().gap_1();
    if settings.upstreams.is_empty() && inputs.upstream_form.is_none() {
        col = col.child(
            Text::new(
                "No upstreams yet. Add one so an agent can call an approved API without ever \
                 seeing the key.",
            )
            .size(Size::Xs)
            .dimmed(),
        );
    }
    for (i, u) in settings.upstreams.iter().enumerate() {
        let present = crate::secrets::has_secret(&u.secret, u.project);
        let (label, color) = if present {
            ("secret set", ColorName::Green)
        } else {
            ("secret missing", ColorName::Red)
        };
        let scope = if u.project == 0 {
            "global keep".to_string()
        } else {
            format!("project {} keep", u.project)
        };
        let controls = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(Badge::new(label).color(color).variant(Variant::Light))
            .child(mut_btn(
                format!("up-edit-{i}"),
                "Edit",
                Variant::Subtle,
                handle,
                move |root, cx| open_upstream_form(root, Some(i), cx),
            ))
            .child(mut_btn(
                format!("up-remove-{i}"),
                "Remove",
                Variant::Subtle,
                handle,
                move |root, cx| {
                    let mut v = root.settings.upstreams.clone();
                    if i < v.len() {
                        v.remove(i);
                    }
                    write_upstreams(root, v, cx);
                },
            ));
        col = col.child(row(
            SharedString::from(u.name.clone()),
            SharedString::from(format!("{} · {} · {}", u.base_url, u.secret, scope)),
            false,
            None,
            controls.into_any_element(),
            chrome,
        ));
    }
    match &inputs.upstream_form {
        Some(form) => col.child(upstream_form_card(form, handle, chrome)),
        None => col.child(div().pt(px(6.0)).child(mut_btn(
            "up-add".to_string(),
            "Add upstream",
            Variant::Default,
            handle,
            |root, cx| open_upstream_form(root, None, cx),
        ))),
    }
}

fn upstream_form_card(
    form: &UpstreamForm,
    handle: &Entity<Root>,
    chrome: Chrome,
) -> gpui::AnyElement {
    let dimmed = chrome.dimmed;
    let title = if form.index.is_some() {
        "Edit upstream"
    } else {
        "New upstream"
    };
    let mut card = form_frame(chrome.border)
        .child(Text::new(title).size(Size::Sm))
        .child(labeled("Name", &form.name, dimmed))
        .child(labeled("Base URL", &form.base_url, dimmed))
        .child(labeled(
            "Secret name (keep entry to inject)",
            &form.secret,
            dimmed,
        ))
        .child(scope_chips(
            "up-scope",
            form.current_project,
            form.project,
            handle,
            |root, project, _cx| {
                if let Some(f) = root
                    .settings_inputs
                    .as_mut()
                    .and_then(|i| i.upstream_form.as_mut())
                {
                    f.project = project;
                }
            },
        ));
    if form.error.is_some() {
        card = card.child(form_error(&form.error));
    }
    card.child(save_cancel("up", handle, save_upstream, |root| {
        if let Some(inputs) = root.settings_inputs.as_mut() {
            inputs.upstream_form = None;
        }
    }))
    .into_any_element()
}

/// The Custom agents section body.
fn custom_agents_body(
    settings: &config::Settings,
    inputs: &Inputs,
    handle: &Entity<Root>,
    chrome: Chrome,
) -> gpui::Div {
    let mut body = section_body();
    body = body.child(
        Text::new(
            "Bring-your-own agents added on top of the built-in catalog. They also appear in \
             Agents above, with a Test button.",
        )
        .size(Size::Xs)
        .dimmed(),
    );
    for (i, a) in settings.custom_agents.iter().enumerate() {
        let detail = if a.args.is_empty() {
            format!("id: {} · {} · {}", a.id, a.program, a.delivery)
        } else {
            format!(
                "id: {} · {} {} · {}",
                a.id,
                a.program,
                a.args.join(" "),
                a.delivery
            )
        };
        let controls = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(mut_btn(
                format!("ca-edit-{i}"),
                "Edit",
                Variant::Subtle,
                handle,
                move |root, cx| open_agent_form(root, Some(i), cx),
            ))
            .child(mut_btn(
                format!("ca-remove-{i}"),
                "Remove",
                Variant::Subtle,
                handle,
                move |root, cx| {
                    let mut v = root.settings.custom_agents.clone();
                    if i < v.len() {
                        v.remove(i);
                    }
                    write_custom_agents(root, v, cx);
                },
            ));
        let title = if a.name.is_empty() {
            a.id.clone()
        } else {
            a.name.clone()
        };
        body = body.child(row(
            SharedString::from(title),
            SharedString::from(detail),
            false,
            None,
            controls.into_any_element(),
            chrome,
        ));
    }
    match &inputs.agent_form {
        Some(form) => body.child(agent_form_card(form, handle, chrome)),
        None => body.child(div().pt(px(6.0)).child(mut_btn(
            "ca-add".to_string(),
            "Add custom agent",
            Variant::Default,
            handle,
            |root, cx| open_agent_form(root, None, cx),
        ))),
    }
}

fn agent_form_card(
    form: &CustomAgentForm,
    handle: &Entity<Root>,
    chrome: Chrome,
) -> gpui::AnyElement {
    let dimmed = chrome.dimmed;
    let title = if form.index.is_some() {
        "Edit custom agent"
    } else {
        "New custom agent"
    };
    let mut card = form_frame(chrome.border)
        .child(Text::new(title).size(Size::Sm))
        .child(labeled("Id", &form.id, dimmed))
        .child(labeled("Name (optional)", &form.name, dimmed))
        .child(labeled("Icon (optional)", &form.icon, dimmed))
        .child(labeled("Program", &form.program, dimmed))
        .child(labeled(
            "Args (use {prompt} for the task)",
            &form.args,
            dimmed,
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(3.0))
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(dimmed)
                        .child("Prompt delivery"),
                )
                .child(chips(
                    "ca-delivery",
                    vec![
                        ("arg".to_string(), "arg".to_string()),
                        ("stdin".to_string(), "stdin".to_string()),
                    ],
                    &form.delivery,
                    handle,
                    |root, v, _cx| {
                        if let Some(f) = root
                            .settings_inputs
                            .as_mut()
                            .and_then(|i| i.agent_form.as_mut())
                        {
                            f.delivery = v;
                        }
                    },
                )),
        );
    if form.error.is_some() {
        card = card.child(form_error(&form.error));
    }
    card.child(save_cancel("ca", handle, save_agent, |root| {
        if let Some(inputs) = root.settings_inputs.as_mut() {
            inputs.agent_form = None;
        }
    }))
    .into_any_element()
}

/// The Layouts section body.
fn layouts_body(
    settings: &config::Settings,
    inputs: &Inputs,
    handle: &Entity<Root>,
    chrome: Chrome,
) -> gpui::Div {
    let mut body = section_body();
    body = body.child(
        Text::new(
            "Named fan-out presets. Picking a layout when composing a task selects its agents \
             (and optional concurrency) in one gesture.",
        )
        .size(Size::Xs)
        .dimmed(),
    );
    for (i, l) in settings.layouts.iter().enumerate() {
        let cc = if l.concurrency == 0 {
            "all at once".to_string()
        } else {
            format!("{} at a time", l.concurrency)
        };
        let detail = format!("{} · {}", l.agents.join(", "), cc);
        let controls = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(mut_btn(
                format!("ly-edit-{i}"),
                "Edit",
                Variant::Subtle,
                handle,
                move |root, cx| open_layout_form(root, Some(i), cx),
            ))
            .child(mut_btn(
                format!("ly-remove-{i}"),
                "Remove",
                Variant::Subtle,
                handle,
                move |root, cx| {
                    let mut v = root.settings.layouts.clone();
                    if i < v.len() {
                        v.remove(i);
                    }
                    write_layouts(root, v, cx);
                },
            ));
        body = body.child(row(
            SharedString::from(l.name.clone()),
            SharedString::from(detail),
            false,
            None,
            controls.into_any_element(),
            chrome,
        ));
    }
    match &inputs.layout_form {
        Some(form) => body.child(layout_form_card(form, handle, chrome)),
        None => body.child(div().pt(px(6.0)).child(mut_btn(
            "ly-add".to_string(),
            "Add layout",
            Variant::Default,
            handle,
            |root, cx| open_layout_form(root, None, cx),
        ))),
    }
}

fn layout_form_card(form: &LayoutForm, handle: &Entity<Root>, chrome: Chrome) -> gpui::AnyElement {
    let dimmed = chrome.dimmed;
    let title = if form.index.is_some() {
        "Edit layout"
    } else {
        "New layout"
    };
    let display = if form.concurrency == 0 {
        "all".to_string()
    } else {
        form.concurrency.to_string()
    };
    let mut card = form_frame(chrome.border)
        .child(Text::new(title).size(Size::Sm))
        .child(labeled("Name", &form.name, dimmed))
        .child(labeled("Description (optional)", &form.description, dimmed))
        .child(labeled(
            "Agent ids (comma or space separated)",
            &form.agents,
            dimmed,
        ))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(Text::new("Concurrency").size(Size::Xs).dimmed())
                .child(stepper(
                    "ly-conc",
                    form.concurrency as i64,
                    (0, 32),
                    display,
                    chrome.border,
                    handle,
                    |root, v, cx| {
                        if let Some(f) = root
                            .settings_inputs
                            .as_mut()
                            .and_then(|i| i.layout_form.as_mut())
                        {
                            f.concurrency = v as u32;
                        }
                        cx.notify();
                    },
                )),
        );
    if form.error.is_some() {
        card = card.child(form_error(&form.error));
    }
    card.child(save_cancel("ly", handle, save_layout, |root| {
        if let Some(inputs) = root.settings_inputs.as_mut() {
            inputs.layout_form = None;
        }
    }))
    .into_any_element()
}

/// The Secrets keep section body: the unlock flow, then the scoped secret list
/// and an add form once unlocked. Values are never displayed.
fn keep_body(inputs: &Inputs, handle: &Entity<Root>, chrome: Chrome) -> gpui::Div {
    let mut body = section_body();
    body = body.child(
        Text::new(
            "The encrypted keep holds the API keys the secrets proxy and MCP gateway inject for \
             agents. Unlock it with your passphrase to manage entries; values are never shown \
             or written to settings.json.",
        )
        .size(Size::Xs)
        .dimmed(),
    );
    match crate::secrets::keep_status() {
        crate::secrets::KeepStatus::Unlocked => {
            body = body.child(row(
                "Status",
                "Unlocked and held in memory for this session.",
                false,
                None,
                Badge::new("unlocked")
                    .color(ColorName::Green)
                    .variant(Variant::Light)
                    .into_any_element(),
                chrome,
            ));
            let scopes = crate::secrets::keep_scopes();
            if scopes.is_empty() && inputs.secret_form.is_none() {
                body = body.child(Text::new("No secrets stored yet.").size(Size::Xs).dimmed());
            }
            for (scope_key, names) in &scopes {
                let project = scope_key_project(scope_key);
                let scope_label = scope_display(scope_key);
                for name in names {
                    let name_owned = name.clone();
                    let control = mut_btn(
                        format!("sec-remove-{scope_key}-{name}"),
                        "Remove",
                        Variant::Subtle,
                        handle,
                        move |root, cx| {
                            if let Err(e) = crate::secrets::keep_remove(project, &name_owned) {
                                root.push_error("Could not remove secret", e);
                            }
                            cx.notify();
                        },
                    );
                    body = body.child(row(
                        SharedString::from(name.clone()),
                        SharedString::from(scope_label.clone()),
                        false,
                        None,
                        control.into_any_element(),
                        chrome,
                    ));
                }
            }
            body = match &inputs.secret_form {
                Some(form) => body.child(secret_form_card(form, handle, chrome)),
                None => body.child(div().pt(px(6.0)).child(mut_btn(
                    "sec-add".to_string(),
                    "Add secret",
                    Variant::Default,
                    handle,
                    open_secret_form,
                ))),
            };
        }
        status => {
            let (desc, action) = if status == crate::secrets::KeepStatus::Missing {
                (
                    "No keep exists yet. Enter a passphrase to create one.",
                    "Create keep",
                )
            } else {
                (
                    "Locked. Enter your passphrase to unlock it for this session.",
                    "Unlock",
                )
            };
            body = body.child(row(
                "Passphrase",
                desc,
                false,
                None,
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .w(px(220.0))
                            .flex_none()
                            .child(inputs.keep_pass.clone()),
                    )
                    .child(mut_btn(
                        "keep-unlock".to_string(),
                        action,
                        Variant::Filled,
                        handle,
                        unlock_keep_action,
                    ))
                    .into_any_element(),
                chrome,
            ));
            if let Some(err) = &inputs.keep_error {
                body = body.child(
                    div().pt(px(4.0)).child(
                        Badge::new(err.clone())
                            .color(ColorName::Red)
                            .variant(Variant::Light),
                    ),
                );
            }
        }
    }
    body
}

fn secret_form_card(form: &SecretForm, handle: &Entity<Root>, chrome: Chrome) -> gpui::AnyElement {
    let dimmed = chrome.dimmed;
    let mut card = form_frame(chrome.border)
        .child(Text::new("New secret").size(Size::Sm))
        .child(labeled("Name", &form.name, dimmed))
        .child(labeled("Value", &form.value, dimmed))
        .child(scope_chips(
            "sec-scope",
            form.current_project,
            form.project,
            handle,
            |root, project, _cx| {
                if let Some(f) = root
                    .settings_inputs
                    .as_mut()
                    .and_then(|i| i.secret_form.as_mut())
                {
                    f.project = project;
                }
            },
        ));
    if form.error.is_some() {
        card = card.child(form_error(&form.error));
    }
    card.child(save_cancel("sec", handle, save_secret, |root| {
        if let Some(inputs) = root.settings_inputs.as_mut() {
            inputs.secret_form = None;
        }
    }))
    .into_any_element()
}

#[cfg(test)]
#[path = "../tests/settings.rs"]
mod tests;
