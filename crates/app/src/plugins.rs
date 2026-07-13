//! The Plugins surface: installed plugins, their contributed commands and
//! capabilities, and any manifest load diagnostics.

use gpui::prelude::*;
use gpui::{div, px, App, IntoElement, SharedString, Window};
use guise::prelude::*;

use plugin::Installed;

pub fn plugins_view(
    installed: Installed,
    dir: String,
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
        return col.child(
            Text::new("No plugins installed. Drop a plugin directory (with a plugin.toml) into the plugins folder.")
                .size(Size::Sm)
                .dimmed(),
        );
    }

    for p in installed.plugins {
        col = col.child(plugin_card(p));
    }
    col
}

fn plugin_card(p: plugin::Plugin) -> impl IntoElement {
    let runtime = match &p.runtime {
        Some(rt) => match rt.kind {
            plugin::RuntimeKind::Wasm => "wasm",
            plugin::RuntimeKind::Process => "process",
        },
        None => "none",
    };

    let mut body = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(Text::new(SharedString::from(p.name.clone())).bold())
                .child(Text::new(SharedString::from(format!("v{}", p.version))).size(Size::Xs).dimmed())
                .child(Badge::new(runtime).variant(Variant::Light)),
        );

    if let Some(desc) = &p.description {
        body = body.child(Text::new(SharedString::from(desc.clone())).size(Size::Sm).dimmed());
    }

    // Capabilities.
    if !p.capabilities.is_empty() {
        let mut caps = div().flex().flex_row().flex_wrap().gap_1();
        for cap in &p.capabilities {
            caps = caps.child(Badge::new(SharedString::from(cap.clone())).color(ColorName::Blue).variant(Variant::Light));
        }
        body = body.child(caps);
    }

    // Commands.
    if !p.commands.is_empty() {
        let mut cmds = div().flex().flex_col().gap_1();
        for c in &p.commands {
            cmds = cmds.child(
                Text::new(SharedString::from(format!("⌘ {}", c.title))).size(Size::Sm),
            );
        }
        body = body.child(Divider::new()).child(cmds);
    }

    Card::new().padding(Size::Md).child(body)
}
