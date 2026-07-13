//! The search surface: a query field over the project, listing file/line hits.
//!
//! The input commits on Enter, running `search::search` against the selected
//! project's directory and storing results on [`Root`].

use gpui::prelude::*;
use gpui::{div, px, App, Entity, IntoElement, SharedString, Window};
use guise::prelude::*;

use crate::state::Root;

pub fn search_view(
    query: String,
    results: Vec<search::Match>,
    handle: Entity<Root>,
    _window: &mut Window,
    _cx: &mut App,
) -> impl IntoElement {
    let mut col = div().flex().flex_col().w_full().gap_4().p(px(20.0));
    col = col.child(Title::new("Search").order(2));

    // A simple submit affordance: the query is shown, run on the button.
    let run = handle.clone();
    col = col.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .px(px(10.0))
                    .py(px(6.0))
                    .border_1()
                    .rounded(px(6.0))
                    .child(
                        Text::new(if query.is_empty() {
                            SharedString::from("Type a pattern, then Run…")
                        } else {
                            SharedString::from(query.clone())
                        })
                        .dimmed(),
                    ),
            )
            .child(
                Button::new("run-search", "Run")
                    .size(Size::Sm)
                    .variant(Variant::Filled)
                    .on_click(move |_, _, cx| {
                        run.update(cx, |root, cx| {
                            root.run_search();
                            cx.notify();
                        });
                    }),
            ),
    );

    if results.is_empty() {
        col = col.child(Text::new("No matches.").size(Size::Sm).dimmed());
    } else {
        col = col.child(
            Text::new(SharedString::from(format!("{} matches", results.len())))
                .size(Size::Xs)
                .dimmed(),
        );
        let mut list = div().flex().flex_col().w_full().font_family("monospace").text_size(px(12.0));
        for m in results.into_iter().take(300) {
            list = list.child(
                div().py(px(2.0)).child(Text::new(SharedString::from(format!(
                    "{}:{}  {}",
                    m.file,
                    m.line,
                    m.text.trim()
                )))),
            );
        }
        col = col.child(list);
    }

    col
}
