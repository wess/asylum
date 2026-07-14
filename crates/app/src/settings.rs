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

/// The surface's text inputs, built lazily and kept in sync with the file.
#[derive(Clone)]
pub struct Inputs {
    pub worktree: Entity<guise::TextInput>,
    pub font: Entity<guise::TextInput>,
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

// ── The surface ─────────────────────────────────────────────────────────────

pub fn settings_view(
    settings: config::Settings,
    diagnostics: Vec<config::Diagnostic>,
    reports: Vec<(agent::registry::Agent, agent::doctor::Report)>,
    inputs: Inputs,
    handle: Entity<Root>,
    _window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let t = guise::theme::theme(cx);
    let dimmed = t.dimmed().hsla();
    let border = t.border().hsla();
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

    // ── General ──
    col = col.child(heading("General", dimmed, border));
    col = col.child(row(
        "Theme",
        "Color scheme for the whole app.",
        settings.theme != defaults.theme,
        Some(reset_btn("reset-theme", "theme", &handle)),
        theme_control(&settings.theme, &handle),
        border,
    ));
    col = col.child(row(
        "Worktree directory",
        "Where per-task worktrees are created, relative to a project root. Press enter to apply.",
        settings.worktree_dir != defaults.worktree_dir,
        Some(reset_btn("reset-worktree", "worktree_dir", &handle)),
        div()
            .w(px(280.0))
            .flex_none()
            .child(inputs.worktree.clone())
            .into_any_element(),
        border,
    ));
    {
        let display = if settings.max_parallel_runs == 0 {
            "unlimited".to_string()
        } else {
            settings.max_parallel_runs.to_string()
        };
        let value = settings.max_parallel_runs as i64;
        col = col.child(row(
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
            border,
        ));
    }
    {
        let display = if settings.run_timeout_minutes == 0 {
            "off".to_string()
        } else {
            format!("{} min", settings.run_timeout_minutes)
        };
        let value = settings.run_timeout_minutes as i64;
        col = col.child(row(
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
            border,
        ));
    }

    // ── Agents ──
    col = col.child(heading("Agents", dimmed, border));
    col = col.child(
        Text::new("Agents fanned out by default when a task is dispatched.")
            .size(Size::Xs)
            .dimmed(),
    );
    let selected = settings.default_agents.clone();
    for agent in agent::registry::catalog(&settings.custom_agents) {
        let on = selected.contains(&agent.id);
        let id = agent.id.clone();
        let h = handle.clone();
        let current = selected.clone();
        let report = reports
            .iter()
            .find(|(candidate, _)| candidate.id == agent.id);
        let (status, color) = match report {
            Some((_, report)) if report.verified() => ("verified", ColorName::Green),
            Some((_, report)) if report.ready() => ("installed", ColorName::Orange),
            _ => ("not found", ColorName::Red),
        };
        let program = inputs.programs.get(&agent.id).cloned();
        let mut controls = div().flex().flex_row().items_center().gap_2();
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
        col = col.child(row(
            SharedString::from(agent.name.clone()),
            SharedString::from(format!("id: {} · executable", agent.id)),
            false,
            None,
            controls.into_any_element(),
            border,
        ));
    }

    // ── Editor ──
    col = col.child(heading("Editor", dimmed, border));
    col = col.child(row(
        "Font family",
        "Editor font; press enter to apply.",
        ed.font_family != ed_defaults.font_family,
        None,
        div()
            .w(px(280.0))
            .flex_none()
            .child(inputs.font.clone())
            .into_any_element(),
        border,
    ));
    col = col.child(row(
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
        border,
    ));
    col = col.child(row(
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
        border,
    ));
    {
        let h = handle.clone();
        let autosave = ed.autosave;
        col = col.child(row(
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
            border,
        ));
    }

    // ── Keybindings ──
    col = col.child(heading("Keybindings", dimmed, border));
    col = col.child(
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
    col = col.child(keys);

    div()
        .id("settings-scroll")
        .size_full()
        .overflow_y_scroll()
        .child(col)
}

/// A group label with its underline divider.
fn heading(text: &'static str, dimmed: gpui::Hsla, border: gpui::Hsla) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .pt(px(22.0))
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(12.5))
                .text_color(dimmed)
                .child(SharedString::from(text)),
        )
        .child(div().w_full().h(px(1.0)).bg(border))
}

/// One settings row: label + description on the left, the control (and a
/// Reset when the value differs from the default) on the right.
fn row(
    label: impl Into<SharedString>,
    desc: impl Into<SharedString>,
    modified: bool,
    reset: Option<gpui::AnyElement>,
    control: gpui::AnyElement,
    border: gpui::Hsla,
) -> impl IntoElement {
    let left = div()
        .flex()
        .flex_col()
        .gap(px(2.0))
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
        .child(Text::new(desc.into()).size(Size::Xs).dimmed());

    let mut right = div().flex().flex_row().items_center().gap_2().flex_none();
    if let Some(reset) = reset.filter(|_| modified) {
        right = right.child(reset);
    }
    right = right.child(control);

    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_3()
        .py(px(10.0))
        .border_b_1()
        .border_color(border)
        .child(left)
        .child(right)
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
