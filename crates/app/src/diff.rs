//! The diff review surface: render a run's changes as an annotatable unified
//! diff. Added lines get a green wash, removed lines a red one, with old/new
//! line numbers in the gutter. Click any line to anchor a comment there;
//! comments render inline under their line and ship back to the agent.

use gpui::prelude::*;
use gpui::{div, px, rgba, App, Entity, IntoElement, SharedString, Window};
use guise::prelude::*;

use crate::control::Button;
use crate::state::Root;
use crate::state::RunRow;
use checks::{CheckResult, Status};
use git::{DiffFile, LineKind};
use store::{Annotation, Side};

/// The diff line a pending comment anchors to: (file, line, side).
pub type Target = Option<(String, u32, Side)>;

/// Build the diff review content from the parsed files.
#[allow(clippy::too_many_arguments)]
pub fn review(
    files: Vec<DiffFile>,
    check_results: Vec<CheckResult>,
    checking: bool,
    annotations: Vec<Annotation>,
    target: Target,
    branches: Vec<git::Branch>,
    runs: Vec<RunRow>,
    split: bool,
    note: Entity<guise::TextInput>,
    handle: Entity<Root>,
    _window: &mut Window,
    _cx: &mut App,
) -> impl IntoElement {
    let mut col = div()
        .id("diff-scroll")
        .flex()
        .flex_col()
        .size_full()
        .gap_4()
        .p(px(20.0))
        .overflow_y_scroll();
    col = col.child(
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_2()
            .justify_between()
            .child(Title::new("Review changes").order(2))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child({
                        let toggle = handle.clone();
                        Button::new(
                            "diff-view-toggle",
                            if split {
                                "Unified view"
                            } else {
                                "Side-by-side"
                            },
                        )
                        .size(Size::Xs)
                        .variant(Variant::Subtle)
                        .on_click(move |_, _, cx| {
                            toggle.update(cx, |root, cx| {
                                root.diff_split = !root.diff_split;
                                cx.notify();
                            });
                        })
                    })
                    .child(checks_bar(check_results, checking, handle.clone())),
            ),
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
            let color = if b.head {
                ColorName::Green
            } else {
                ColorName::Gray
            };
            chips = chips.child(
                Badge::new(SharedString::from(b.name))
                    .color(color)
                    .variant(Variant::Light),
            );
        }
        col = col.child(chips);
    }

    if !runs.is_empty() {
        let mut row = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_1()
            .child(Text::new("Compare:").size(Size::Xs).dimmed());
        for run in runs {
            let select = handle.clone();
            let id = run.id;
            let name = agent::find(&run.agent)
                .map(|agent| agent.name)
                .unwrap_or(run.agent.as_str());
            row = row.child(
                Button::new(
                    SharedString::from(format!("diff-run-{id}")),
                    SharedString::from(format!("{}{}", if run.selected { "* " } else { "" }, name)),
                )
                .size(Size::Xs)
                .variant(if run.selected {
                    Variant::Filled
                } else {
                    Variant::Light
                })
                .on_click(move |_, _, cx| {
                    select.update(cx, |root, cx| {
                        root.select_run(id);
                        cx.notify();
                    });
                }),
            );
        }
        col = col.child(row);
    }

    col = col.child(comment_panel(
        &annotations,
        target.clone(),
        note,
        handle.clone(),
    ));

    if files.is_empty() {
        return col.child(
            Text::new("No changes to review yet.")
                .size(Size::Sm)
                .dimmed(),
        );
    }

    for file in files {
        col = if split {
            col.child(file_block_split(file))
        } else {
            col.child(file_block(file, &annotations, &target, handle.clone()))
        };
    }
    col
}

/// The review-comment panel: where the next comment lands, the input to add
/// it, and the button that ships the open batch back to an agent.
fn comment_panel(
    annotations: &[Annotation],
    target: Target,
    note: Entity<guise::TextInput>,
    handle: Entity<Root>,
) -> impl IntoElement {
    let mut panel = div().flex().flex_col().gap_2();

    let open = annotations.iter().filter(|a| !a.resolved).count();
    let anchor = match &target {
        Some((file, line, _)) => format!("Commenting on {file}:{line}"),
        None => "Click a diff line to anchor your comment.".to_string(),
    };
    let mut status = div().flex().flex_row().items_center().gap_2().child(
        Text::new(SharedString::from(anchor))
            .size(Size::Xs)
            .dimmed(),
    );
    if open > 0 {
        let send = handle.clone();
        status = status
            .child(
                Badge::new(SharedString::from(format!("{open} open")))
                    .color(ColorName::Blue)
                    .variant(Variant::Light),
            )
            .child(
                Button::new("send-review", "Send review to agent")
                    .size(Size::Xs)
                    .variant(Variant::Filled)
                    .on_click(move |_, window, cx| {
                        send.update(cx, |root, cx| {
                            root.send_review_to_agent(window, cx);
                            cx.notify();
                        });
                    }),
            );
    }
    panel = panel.child(status);

    let add = handle;
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
fn checks_bar(results: Vec<CheckResult>, checking: bool, handle: Entity<Root>) -> impl IntoElement {
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
        Button::new(
            "run-checks",
            if checking {
                "Checks running"
            } else {
                "Run checks"
            },
        )
        .size(Size::Xs)
        .variant(Variant::Filled)
        .disabled(checking)
        .on_click(move |_, _, cx| {
            handle.update(cx, |root, cx| {
                root.run_checks(cx);
                cx.notify();
            });
        }),
    );
    row
}

fn file_block(
    file: DiffFile,
    annotations: &[Annotation],
    target: &Target,
    handle: Entity<Root>,
) -> impl IntoElement {
    let (added, removed) = file.line_stats();
    let header = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(Text::new(SharedString::from(file.path.clone())).bold())
        .child(
            Badge::new(format!("+{added}"))
                .color(ColorName::Green)
                .variant(Variant::Light),
        )
        .child(
            Badge::new(format!("-{removed}"))
                .color(ColorName::Red)
                .variant(Variant::Light),
        );

    let mut body = div()
        .flex()
        .flex_col()
        .w_full()
        .font_family("monospace")
        .text_size(px(12.0));

    for (hi, hunk) in file.hunks.iter().enumerate() {
        body = body.child(
            div().px(px(8.0)).py(px(2.0)).text_size(px(11.0)).child(
                Text::new(SharedString::from(format!(
                    "@@ -{} +{} @@ {}",
                    hunk.old_start, hunk.new_start, hunk.header
                )))
                .dimmed(),
            ),
        );
        for (li, line) in hunk.lines.iter().enumerate() {
            let (line_no, side) = anchor_of(line);
            let targeted = matches!(
                (target, line_no),
                (Some((f, l, s)), Some(n)) if *f == file.path && *l == n && *s == side
            );
            body = body.child(diff_line(
                line,
                SharedString::from(format!("dl-{}-{hi}-{li}", file.path)),
                file.path.clone(),
                line_no,
                side,
                targeted,
                handle.clone(),
            ));
            if let Some(n) = line_no {
                for a in annotations
                    .iter()
                    .filter(|a| a.file == file.path && a.line == n && a.side == side)
                {
                    body = body.child(annotation_row(a, handle.clone()));
                }
            }
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

/// A read-only side-by-side rendering of one file: removed lines on the left,
/// added lines on the right, context spanning both. Annotating stays in the
/// unified view, which owns the click-to-comment interaction.
fn file_block_split(file: DiffFile) -> impl IntoElement {
    let (added, removed) = file.line_stats();
    let header = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(Text::new(SharedString::from(file.path.clone())).bold())
        .child(
            Badge::new(format!("+{added}"))
                .color(ColorName::Green)
                .variant(Variant::Light),
        )
        .child(
            Badge::new(format!("-{removed}"))
                .color(ColorName::Red)
                .variant(Variant::Light),
        );

    let mut body = div()
        .flex()
        .flex_col()
        .w_full()
        .font_family("monospace")
        .text_size(px(12.0));

    for hunk in &file.hunks {
        body = body.child(
            div().px(px(8.0)).py(px(2.0)).text_size(px(11.0)).child(
                Text::new(SharedString::from(format!(
                    "@@ -{} +{} @@ {}",
                    hunk.old_start, hunk.new_start, hunk.header
                )))
                .dimmed(),
            ),
        );
        // Pair each run of removed lines with the following added lines so a
        // change shows old and new side by side; context flushes the pairing.
        let mut removed_run: Vec<&git::DiffLine> = Vec::new();
        let mut added_run: Vec<&git::DiffLine> = Vec::new();
        let flush = |body: gpui::Div,
                     removed_run: &mut Vec<&git::DiffLine>,
                     added_run: &mut Vec<&git::DiffLine>| {
            let rows = removed_run.len().max(added_run.len());
            let mut body = body;
            for i in 0..rows {
                body = body.child(split_row(
                    removed_run.get(i).copied(),
                    added_run.get(i).copied(),
                ));
            }
            removed_run.clear();
            added_run.clear();
            body
        };
        for line in &hunk.lines {
            match line.kind {
                LineKind::Removed => removed_run.push(line),
                LineKind::Added => added_run.push(line),
                LineKind::Context => {
                    body = flush(body, &mut removed_run, &mut added_run);
                    body = body.child(split_row(Some(line), Some(line)));
                }
            }
        }
        body = flush(body, &mut removed_run, &mut added_run);
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

/// One row of the side-by-side view: an old cell and a new cell.
fn split_row(left: Option<&git::DiffLine>, right: Option<&git::DiffLine>) -> impl IntoElement {
    let cell = |line: Option<&git::DiffLine>, removed: bool| {
        let (bg, no, content) = match line {
            Some(l) if removed && l.kind == LineKind::Removed => {
                (rgba(0xf8514926), l.old_no, l.content.clone())
            }
            Some(l) if !removed && l.kind == LineKind::Added => {
                (rgba(0x2ea04326), l.new_no, l.content.clone())
            }
            Some(l) if l.kind == LineKind::Context => (
                rgba(0x00000000),
                if removed { l.old_no } else { l.new_no },
                l.content.clone(),
            ),
            _ => (rgba(0x00000000), None, String::new()),
        };
        let gutter = match no {
            Some(v) => format!("{v:>4}"),
            None => "    ".to_string(),
        };
        div()
            .flex()
            .flex_row()
            .w_1_2()
            .bg(bg)
            .child(
                div()
                    .px(px(6.0))
                    .child(Text::new(SharedString::from(gutter)).dimmed()),
            )
            .child(Text::new(SharedString::from(content)))
    };
    div()
        .flex()
        .flex_row()
        .w_full()
        .gap_2()
        .child(cell(left, true))
        .child(cell(right, false))
}

/// Which (line number, side) a comment on this line anchors to.
fn anchor_of(line: &git::DiffLine) -> (Option<u32>, Side) {
    match line.kind {
        LineKind::Removed => (line.old_no, Side::Old),
        _ => (line.new_no, Side::New),
    }
}

#[allow(clippy::too_many_arguments)]
fn diff_line(
    line: &git::DiffLine,
    id: SharedString,
    file: String,
    line_no: Option<u32>,
    side: Side,
    targeted: bool,
    handle: Entity<Root>,
) -> impl IntoElement {
    let (bg, sign) = match line.kind {
        LineKind::Added => (rgba(0x2ea04326), "+"),
        LineKind::Removed => (rgba(0xf8514926), "-"),
        LineKind::Context => (rgba(0x00000000), " "),
    };
    let gutter = |n: Option<u32>| match n {
        Some(v) => format!("{v:>4}"),
        None => "    ".to_string(),
    };
    let mut row = div()
        .id(id)
        .flex()
        .flex_row()
        .w_full()
        .bg(if targeted { rgba(0x3b82f633) } else { bg })
        .cursor_pointer()
        .child(
            div().px(px(6.0)).child(
                Text::new(SharedString::from(format!(
                    "{} {} ",
                    gutter(line.old_no),
                    gutter(line.new_no)
                )))
                .dimmed(),
            ),
        )
        .child(Text::new(SharedString::from(format!(
            "{sign} {}",
            line.content
        ))));
    if let Some(n) = line_no {
        row = row
            .tab_index(0)
            .role(gpui::accesskit::Role::Button)
            .aria_label(SharedString::from(format!("Comment on {file} line {n}")))
            .focus_visible(|style| style.border_1().border_color(gpui::rgb(0x3b82f6)))
            .on_click(move |_, _, cx| {
                handle.update(cx, |root, cx| {
                    root.target_review_line(&file, n, side);
                    cx.notify();
                });
            });
    }
    row
}

/// An inline review comment under its diff line, with resolve/delete.
fn annotation_row(a: &Annotation, handle: Entity<Root>) -> impl IntoElement {
    let (id, resolved) = (a.id, a.resolved);
    let body = if resolved {
        Text::new(SharedString::from(format!("💬 {}", a.body)))
            .size(Size::Xs)
            .dimmed()
    } else {
        Text::new(SharedString::from(format!("💬 {}", a.body))).size(Size::Xs)
    };
    let resolve = handle.clone();
    let del = handle;
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .pl(px(56.0))
        .pr(px(8.0))
        .py(px(3.0))
        .bg(rgba(0x3b82f61a))
        .child(div().flex_1().overflow_hidden().child(body))
        .child(
            div()
                .id(SharedString::from(format!("ann-res-{id}")))
                .px(px(4.0))
                .cursor_pointer()
                .tab_index(0)
                .role(gpui::accesskit::Role::Button)
                .aria_label(if resolved {
                    "Reopen comment"
                } else {
                    "Resolve comment"
                })
                .focus_visible(|style| style.border_1().border_color(gpui::rgb(0x3b82f6)))
                .child(
                    Text::new(if resolved { "↺" } else { "✓" })
                        .size(Size::Xs)
                        .dimmed(),
                )
                .on_click(move |_, _, cx| {
                    resolve.update(cx, |root, cx| {
                        root.resolve_review_note(id, !resolved);
                        cx.notify();
                    });
                }),
        )
        .child(
            div()
                .id(SharedString::from(format!("ann-del-{id}")))
                .px(px(4.0))
                .cursor_pointer()
                .tab_index(0)
                .role(gpui::accesskit::Role::Button)
                .aria_label("Delete comment")
                .focus_visible(|style| style.border_1().border_color(gpui::rgb(0x3b82f6)))
                .child(Text::new("×").size(Size::Xs).dimmed())
                .on_click(move |_, _, cx| {
                    del.update(cx, |root, cx| {
                        root.delete_review_note(id);
                        cx.notify();
                    });
                }),
        )
}
