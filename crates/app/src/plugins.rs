//! The Plugins surface: installed plugins, their enable/disable trust control,
//! contributed commands and triggers, and manifest load diagnostics. Only the
//! contribution types the host actually drives (`[[command]]`, `[[trigger]]`)
//! are presented as live; declared `[panel]`/`[webview]`/`[[tool]]` are labeled
//! "not yet active" so the surface never advertises something that does nothing.

use gpui::prelude::*;
use gpui::{div, px, App, Entity, IntoElement, SharedString, Window};
use guise::prelude::*;

use crate::control::{empty, Button};
use crate::state::Root;
use plugin::{Installed, TriggerAction};

pub fn plugins_view(
    installed: Installed,
    dir: String,
    enabled: Vec<String>,
    handle: Entity<Root>,
    _window: &mut Window,
    _cx: &mut App,
) -> impl IntoElement {
    let mut col = div()
        .id("plugins-scroll")
        .flex()
        .flex_col()
        .size_full()
        .gap_4()
        .p(px(20.0))
        .overflow_y_scroll();
    col = col.child(Title::new("Plugins").order(2));
    col = col.child(
        Text::new(SharedString::from(format!("Directory: {dir}")))
            .size(Size::Xs)
            .dimmed(),
    );

    for d in &installed.diagnostics {
        col = col.child(
            Alert::new(SharedString::from(format!(
                "{}: {}",
                d.path.display(),
                d.message
            )))
            .color(ColorName::Yellow),
        );
    }

    if installed.plugins.is_empty() {
        return col.child(empty(
            "Make Asylum your own",
            "No plugins are installed yet. Add a plugin folder containing plugin.toml to the directory shown above.",
        ));
    }

    for p in installed.plugins {
        let is_enabled = enabled.iter().any(|e| e == &p.id);
        col = col.child(plugin_card(p, is_enabled, handle.clone()));
    }
    col
}

fn plugin_card(p: plugin::Plugin, enabled: bool, handle: Entity<Root>) -> impl IntoElement {
    let runtime = match &p.runtime {
        Some(rt) => match rt.kind {
            plugin::RuntimeKind::Wasm => "wasm",
            plugin::RuntimeKind::Process => "process",
        },
        None => "none",
    };

    let mut body = div().flex().flex_col().gap_2().child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(Text::new(SharedString::from(p.name.clone())).bold())
            .child(
                Text::new(SharedString::from(format!("v{}", p.version)))
                    .size(Size::Xs)
                    .dimmed(),
            )
            .child(Badge::new(runtime).variant(Variant::Light))
            .child(
                Badge::new(if enabled { "Enabled" } else { "Disabled" })
                    .color(if enabled {
                        ColorName::Green
                    } else {
                        ColorName::Gray
                    })
                    .variant(Variant::Light),
            )
            .child(div().flex_1())
            .child(enable_button(&p, enabled, handle.clone())),
    );

    if let Some(desc) = &p.description {
        body = body.child(
            Text::new(SharedString::from(desc.clone()))
                .size(Size::Sm)
                .dimmed(),
        );
    }

    // Trust disclosure: a process runtime runs fully trusted (no sandbox), so
    // spell out what it does and with what authority — kept prominent because
    // enabling it is a real trust decision.
    if let Some(rt) = &p.runtime {
        if rt.kind.is_trusted() {
            body = body
                .child(Alert::new(SharedString::from(rt.trust_summary())).color(ColorName::Yellow));
        }
    }

    // Capabilities.
    if !p.capabilities.is_empty() {
        let mut caps = div().flex().flex_row().flex_wrap().gap_1();
        for cap in &p.capabilities {
            caps = caps.child(
                Badge::new(SharedString::from(cap.clone()))
                    .color(ColorName::Blue)
                    .variant(Variant::Light),
            );
        }
        body = body.child(caps);
    }

    // Triggers. These now dispatch on ADE events — but only while the plugin is
    // enabled, so show each hook with its live/inert state.
    if !p.triggers.is_empty() {
        let mut section = div()
            .flex()
            .flex_col()
            .gap_1()
            .child(Text::new("Triggers").size(Size::Xs).dimmed());
        for t in &p.triggers {
            let action = match &t.action {
                TriggerAction::Notify { .. } => "notify".to_string(),
                TriggerAction::Invoke { method } => format!("invoke {method}"),
            };
            let when = t
                .when
                .as_deref()
                .map(|w| format!(" when {w}"))
                .unwrap_or_default();
            section = section.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        Text::new(SharedString::from(format!("on {}{when} → {action}", t.on)))
                            .size(Size::Sm),
                    )
                    .child(
                        Badge::new(if enabled { "active" } else { "inactive" })
                            .color(if enabled {
                                ColorName::Green
                            } else {
                                ColorName::Gray
                            })
                            .variant(Variant::Light),
                    ),
            );
        }
        body = body.child(Divider::new()).child(section);
    }

    // Commands. Runnable only when the plugin is enabled and declares a runtime.
    if !p.commands.is_empty() {
        let runnable = enabled && p.runtime.is_some();
        let mut cmds = div().flex().flex_col().gap_1();
        for c in &p.commands {
            let mut row = div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_2()
                .child(Text::new(SharedString::from(format!("⌘ {}", c.title))).size(Size::Sm));
            if runnable {
                let run = handle.clone();
                let plugin_id = p.id.clone();
                let method = c.run.clone();
                row = row.child(
                    Button::new(SharedString::from(format!("run-{}-{}", p.id, c.id)), "Run")
                        .size(Size::Xs)
                        .variant(Variant::Light)
                        .on_click(move |_, _, cx| {
                            let plugin_id = plugin_id.clone();
                            let method = method.clone();
                            run.update(cx, |root, cx| {
                                root.run_plugin_command(&plugin_id, &method, cx);
                            });
                        }),
                );
            }
            cmds = cmds.child(row);
        }
        body = body.child(Divider::new()).child(cmds);
    }

    // Declared-but-inert contributions: parsed and validated, but the host does
    // not render or expose them yet. Say so instead of implying they work.
    let mut inert = div().flex().flex_col().gap_1();
    let mut any_inert = false;
    if let Some(panel) = &p.panel {
        any_inert = true;
        inert = inert.child(inert_row(format!("Panel: {}", panel.title)));
    }
    if let Some(webview) = &p.webview {
        any_inert = true;
        inert = inert.child(inert_row(format!("Webview: {}", webview.title)));
    }
    for tool in &p.tools {
        any_inert = true;
        inert = inert.child(inert_row(format!("Tool: {}", tool.id)));
    }
    if any_inert {
        body = body.child(Divider::new()).child(inert);
    }

    Card::new().padding(Size::Md).child(body)
}

/// The per-plugin Enable/Disable control. Enabling a trusted process plugin
/// routes through the confirm bar (`request_enable_plugin`); a sandboxed or
/// runtime-less plugin enables in place. Disabling is immediate.
fn enable_button(p: &plugin::Plugin, enabled: bool, handle: Entity<Root>) -> impl IntoElement {
    let id = p.id.clone();
    let label = if enabled { "Disable" } else { "Enable" };
    Button::new(SharedString::from(format!("plugin-toggle-{}", p.id)), label)
        .size(Size::Xs)
        .variant(if enabled {
            Variant::Subtle
        } else {
            Variant::Light
        })
        .on_click(move |_, _, cx| {
            let id = id.clone();
            handle.update(cx, |root, cx| {
                if enabled {
                    root.disable_plugin(&id, cx);
                } else {
                    root.request_enable_plugin(&id, cx);
                }
            });
        })
}

/// A dimmed row for a declared-but-not-yet-active contribution.
fn inert_row(label: String) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_2()
        .child(Text::new(SharedString::from(label)).size(Size::Sm).dimmed())
        .child(
            Badge::new("not yet active")
                .color(ColorName::Gray)
                .variant(Variant::Light),
        )
}
