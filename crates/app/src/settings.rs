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
#[derive(Clone)]
pub struct Inputs {
    pub worktree: Entity<guise::TextInput>,
    pub font: Entity<guise::TextInput>,
    pub proxy_bind: Entity<guise::TextInput>,
    pub mcp_bind: Entity<guise::TextInput>,
    pub programs: std::collections::BTreeMap<String, Entity<guise::TextInput>>,
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
    root.settings_inputs = Some(Inputs {
        worktree,
        font,
        proxy_bind,
        mcp_bind,
        programs,
    });
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
    // Upstreams (edit the list in settings.json). Each shows whether its secret
    // is currently provided in the environment.
    if settings.upstreams.is_empty() {
        body = body.child(
            Text::new(
                "Upstreams let agents call approved external APIs without seeing their secrets. None are configured; add one under \"upstreams\" in settings.json.",
            )
            .size(Size::Xs)
            .dimmed(),
        );
    } else {
        for u in &settings.upstreams {
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
            body = body.child(row(
                SharedString::from(u.name.clone()),
                SharedString::from(format!("{} · {} · {}", u.base_url, u.secret, scope)),
                false,
                None,
                Badge::new(label)
                    .color(color)
                    .variant(Variant::Light)
                    .into_any_element(),
                chrome,
            ));
        }
    }
    col = col.child(section(
        "proxy", "Secrets proxy", &collapsed, body, &handle, dimmed, border,
    ));

    // ── MCP gateway ──
    let mut body = section_body();
    body = body.child(
        Text::new(
            "One MCP server every agent connects to, fronting the servers below under \
             per-service namespaces (github__create_pr). Loopback-only and scoped per \
             run. Edit the server list under \"mcp_servers\" in settings.json. \
             See docs/mcp.md.",
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
    if settings.mcp_servers.is_empty() {
        body = body.child(
            Text::new(
                "No MCP servers configured. Add one under \"mcp_servers\" in settings.json \
                 — a stdio command or an http url — and it appears here, namespaced.",
            )
            .size(Size::Xs)
            .dimmed(),
        );
    } else {
        for server in &settings.mcp_servers {
            let (detail, status, color) = mcp_server_status(server);
            body = body.child(row(
                SharedString::from(server.name.clone()),
                SharedString::from(detail),
                false,
                None,
                Badge::new(status)
                    .color(color)
                    .variant(Variant::Light)
                    .into_any_element(),
                chrome,
            ));
        }
    }
    col = col.child(section(
        "mcp", "MCP gateway", &collapsed, body, &handle, dimmed, border,
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
        "keys", "Keybindings", &collapsed, body, &handle, dimmed, border,
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
pub const SECTIONS: [&str; 6] = ["general", "agents", "editor", "proxy", "mcp", "keys"];

/// The collapse state a fresh Settings surface opens with: every section except
/// the first is collapsed, so the page opens compact.
pub fn default_collapsed() -> std::collections::HashSet<&'static str> {
    SECTIONS.iter().copied().skip(1).collect()
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
