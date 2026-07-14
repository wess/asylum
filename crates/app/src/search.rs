//! Unified project search across source, notes, task prompts, runs, and saved
//! terminal transcripts.

use gpui::prelude::*;
use gpui::{div, px, App, Context, Entity, IntoElement, SharedString, Window};
use guise::prelude::*;
use guise::TextInputEvent;

use crate::control::Button;
use crate::state::{Root, SearchResult};
use crate::workspace::TabKind;

pub fn ensure_input(root: &mut Root, cx: &mut Context<Root>) {
    if root.search_input.is_some() {
        return;
    }
    let input = cx.new(|cx| {
        guise::TextInput::new(cx).placeholder("Search notes, code, tasks, runs, and transcripts")
    });
    cx.subscribe(&input, |root, _input, event: &TextInputEvent, cx| {
        match event {
            TextInputEvent::Change(query) => root.search_query = query.clone(),
            TextInputEvent::Submit(query) => {
                root.search_query = query.clone();
                root.run_search();
            }
        }
        cx.notify();
    })
    .detach();
    root.search_input = Some(input);
    root.run_search();
}

pub fn search_view(
    query: String,
    results: Vec<SearchResult>,
    input: Entity<guise::TextInput>,
    handle: Entity<Root>,
    _window: &mut Window,
    _cx: &mut App,
) -> impl IntoElement {
    let run = handle.clone();
    let mut col = div()
        .id("project-search-scroll")
        .flex()
        .flex_col()
        .size_full()
        .gap_3()
        .p(px(18.0))
        .overflow_y_scroll()
        .child(Title::new("Search").order(2))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(div().flex_1().child(input))
                .child(
                    Button::new("run-search", "Search")
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
        return col
            .child(
                Text::new(if query.trim().is_empty() {
                    "No notes, tasks, or runs yet."
                } else {
                    "No matches in this project."
                })
                .size(Size::Sm)
                .dimmed(),
            )
            .into_any_element();
    }

    col = col.child(
        Text::new(SharedString::from(format!("{} results", results.len())))
            .size(Size::Xs)
            .dimmed(),
    );
    for (index, result) in results.into_iter().enumerate().take(400) {
        let open = handle.clone();
        let row = match &result {
            SearchResult::File(found) => resultrow(
                index,
                "File",
                &format!("{}:{}", found.file, found.line),
                found.text.trim(),
            ),
            SearchResult::Note(found) => resultrow(index, "Note", &found.title, &found.snippet),
            SearchResult::Record(found) => {
                let kind = match found.kind {
                    store::SearchKind::Task => "Task",
                    store::SearchKind::Run => "Run",
                };
                resultrow(index, kind, &found.title, &found.detail)
            }
        };
        col = col.child(row.on_click(move |_, _, cx| {
            let result = result.clone();
            open.update(cx, |root, cx| {
                match result {
                    SearchResult::File(found) => root.open_file(&found.file, cx),
                    SearchResult::Note(found) => {
                        root.open_kind(TabKind::Notes);
                        root.select_note(&found.path, cx);
                    }
                    SearchResult::Record(found) => match found.kind {
                        store::SearchKind::Task => {
                            root.task_id = Some(found.id);
                            root.selected_run_id = root
                                .db
                                .runs(found.id)
                                .ok()
                                .and_then(|runs| runs.first().map(|run| run.id));
                            root.open_kind(TabKind::Tasks);
                        }
                        store::SearchKind::Run => {
                            root.select_run(found.id);
                            root.open_run_terminal(found.id);
                        }
                    },
                }
                cx.notify();
            });
        }));
    }
    col.into_any_element()
}

fn resultrow(index: usize, kind: &str, title: &str, detail: &str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(format!("search-result-{index}")))
        .flex()
        .flex_col()
        .gap_1()
        .w_full()
        .px(px(10.0))
        .py(px(8.0))
        .rounded(px(5.0))
        .border_1()
        .cursor_pointer()
        .tab_index(0)
        .role(gpui::accesskit::Role::Button)
        .aria_label(SharedString::from(format!("Open {kind} {title}")))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(Badge::new(SharedString::from(kind.to_string())))
                .child(Text::new(SharedString::from(title.to_string())).size(Size::Sm)),
        )
        .child(
            Text::new(SharedString::from(
                detail.trim().chars().take(260).collect::<String>(),
            ))
            .size(Size::Xs)
            .dimmed(),
        )
}
