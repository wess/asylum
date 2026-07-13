//! The diff review surface: render a run's changes as an annotatable unified
//! diff. Added lines get a green wash, removed lines a red one, with old/new
//! line numbers in the gutter — the base for inline review comments.

use gpui::prelude::*;
use gpui::{div, px, rgba, App, Entity, IntoElement, SharedString, Window};
use guise::prelude::*;

use crate::state::Root;
use checks::{CheckResult, Status};
use git::{DiffFile, LineKind};
use store::Annotation;

/// Build the diff review content from the parsed files.
#[allow(clippy::too_many_arguments)]
pub fn review(
    files: Vec<DiffFile>,
    check_results: Vec<CheckResult>,
    annotations: Vec<Annotation>,
    branches: Vec<git::Branch>,
    note: Entity<guise::TextInput>,
    handle: Entity<Root>,
    _window: &mut Window,
    _cx: &mut App,
) -> impl IntoElement {
    let mut col = div().flex().flex_col().w_full().gap_4().p(px(20.0));
    col = col.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(Title::new("Review changes").order(2))
            .child(checks_bar(check_results, handle.clone())),
    );

    if !branches.is_empty() {
        let mut chips = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_1()
            .child(Text::new("Branches:").size(Size::Xs).dimmed());
        for b in branches.into_iter().take(12) {
            let color = if b.head { ColorName::Green } else { ColorName::Gray };
            chips = chips.child(
                Badge::new(SharedString::from(b.name)).color(color).variant(Variant::Light),
            );
        }
        col = col.child(chips);
    }

    col = col.child(annotations_panel(annotations, note, handle));

    if files.is_empty() {
        return col.child(
            Text::new("No changes to review yet.")
                .size(Size::Sm)
                .dimmed(),
        );
    }

    for file in files {
        col = col.child(file_block(file));
    }
    col
}

/// The review-comment panel: existing annotations, an input to add a comment,
/// and a button to ship the whole batch back to an agent.
fn annotations_panel(
    annotations: Vec<Annotation>,
    note: Entity<guise::TextInput>,
    handle: Entity<Root>,
) -> impl IntoElement {
    let mut panel = div().flex().flex_col().gap_2();

    if !annotations.is_empty() {
        let mut list = div().flex().flex_col().gap_1();
        for a in &annotations {
            list = list.child(
                Text::new(SharedString::from(format!("💬 {}:{} — {}", a.file, a.line, a.body)))
                    .size(Size::Sm),
            );
        }
        let send = handle.clone();
        panel = panel.child(list).child(
            Button::new("send-review", "Send review to agent")
                .size(Size::Xs)
                .variant(Variant::Filled)
                .on_click(move |_, _, cx| {
                    send.update(cx, |root, cx| {
                        root.send_review_to_agent();
                        cx.notify();
                    });
                }),
        );
    }

    let add = handle.clone();
    let note_read = note.clone();
    panel = panel.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(div().flex_1().child(note))
            .child(
                Button::new("add-note", "Add comment")
                    .size(Size::Xs)
                    .variant(Variant::Light)
                    .on_click(move |_, _, cx| {
                        let text = note_read.read(cx).text();
                        add.update(cx, |root, cx| {
                            root.add_review_note(&text);
                            cx.notify();
                        });
                        note_read.update(cx, |n, cx| n.set_text("", cx));
                    }),
            ),
    );

    Card::new().padding(Size::Sm).child(panel)
}

/// The checks bar: a Run button plus a PASS/FAIL badge per check.
fn checks_bar(results: Vec<CheckResult>, handle: Entity<Root>) -> impl IntoElement {
    let mut row = div().flex().flex_row().items_center().gap_2();
    for r in &results {
        let (color, label) = match r.status {
            Status::Pass => (ColorName::Green, "PASS"),
            Status::Fail => (ColorName::Red, "FAIL"),
            Status::Skipped => (ColorName::Gray, "skip"),
        };
        row = row.child(
            Badge::new(SharedString::from(format!("{} {label}", r.id)))
                .color(color)
                .variant(Variant::Light),
        );
    }
    row = row.child(
        Button::new("run-checks", "Run checks")
            .size(Size::Xs)
            .variant(Variant::Filled)
            .on_click(move |_, _, cx| {
                handle.update(cx, |root, cx| {
                    root.run_checks();
                    cx.notify();
                });
            }),
    );
    row
}

fn file_block(file: DiffFile) -> impl IntoElement {
    let (added, removed) = file.line_stats();
    let header = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(Text::new(SharedString::from(file.path.clone())).bold())
        .child(Badge::new(format!("+{added}")).color(ColorName::Green).variant(Variant::Light))
        .child(Badge::new(format!("-{removed}")).color(ColorName::Red).variant(Variant::Light));

    let mut body = div()
        .flex()
        .flex_col()
        .w_full()
        .font_family("monospace")
        .text_size(px(12.0));

    for hunk in &file.hunks {
        body = body.child(
            div()
                .px(px(8.0))
                .py(px(2.0))
                .text_size(px(11.0))
                .child(Text::new(SharedString::from(format!(
                    "@@ -{} +{} @@ {}",
                    hunk.old_start, hunk.new_start, hunk.header
                )))
                .dimmed()),
        );
        for line in &hunk.lines {
            body = body.child(diff_line(line));
        }
    }

    Card::new().padding(Size::Md).child(
        div()
            .flex()
            .flex_col()
            .w_full()
            .gap_2()
            .child(header)
            .child(Divider::new())
            .child(body),
    )
}

fn diff_line(line: &git::DiffLine) -> impl IntoElement {
    let (bg, sign) = match line.kind {
        LineKind::Added => (rgba(0x2ea04326), "+"),
        LineKind::Removed => (rgba(0xf8514926), "-"),
        LineKind::Context => (rgba(0x00000000), " "),
    };
    let gutter = |n: Option<u32>| match n {
        Some(v) => format!("{v:>4}"),
        None => "    ".to_string(),
    };
    div()
        .flex()
        .flex_row()
        .w_full()
        .bg(bg)
        .child(
            div()
                .px(px(6.0))
                .child(Text::new(SharedString::from(format!(
                    "{} {} ",
                    gutter(line.old_no),
                    gutter(line.new_no)
                )))
                .dimmed()),
        )
        .child(Text::new(SharedString::from(format!("{sign} {}", line.content))))
}
