//! The Plugins surface: installed plugins, their contributed commands and
//! capabilities, and any manifest load diagnostics.

use gpui::prelude::*;
use gpui::{div, px, App, Entity, IntoElement, SharedString, Window};
use guise::prelude::*;

use crate::control::{empty, Button};
use crate::state::Root;
use plugin::Installed;

pub fn plugins_view(
    installed: Installed,
    dir: String,
    handle: Entity<Root>,
    _window: &mut Window,
    _cx: &mut App,
) -> impl IntoElement {
    let mut col = div().flex().flex_col().w_full().gap_4().p(px(20.0));
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
        col = col.child(plugin_card(p, handle.clone()));
    }
    col
}

fn plugin_card(p: plugin::Plugin, handle: Entity<Root>) -> impl IntoElement {
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
            .child(Badge::new(runtime).variant(Variant::Light)),
    );

    if let Some(desc) = &p.description {
        body = body.child(
            Text::new(SharedString::from(desc.clone()))
                .size(Size::Sm)
                .dimmed(),
        );
    }

    // Trust disclosure: a process runtime runs fully trusted (no sandbox), so
    // spell out what it does and with what authority.
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

    // Commands. Each is runnable when the plugin declares a runtime.
    if !p.commands.is_empty() {
        let runnable = p.runtime.is_some();
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

    Card::new().padding(Size::Md).child(body)
}
