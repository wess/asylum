//! Per-run cards on the fan-out board.

use gpui::prelude::*;
use gpui::{div, px, relative, Entity, IntoElement, SharedString};
use guise::prelude::*;

use crate::control::Button;
use crate::state::{Root, RunRow};
use store::RunStatus;

/// A live semantic-activity chip - which agent is working, blocked on input, or
/// done - shown only while the run is active. This is the "which of my agents
/// needs me right now" signal, distinct from the lifecycle status badge.
fn activity_chip(run: &RunRow) -> Option<impl IntoElement> {
    if run.status != RunStatus::Running {
        return None;
    }
    let (label, color) = match run.activity.as_deref()? {
        "blocked" => ("blocked", ColorName::Orange),
        "working" => ("working", ColorName::Blue),
        "done" => ("done", ColorName::Green),
        _ => ("idle", ColorName::Gray),
    };
    Some(Badge::new(label).color(color).variant(Variant::Light))
}

pub(super) fn run_card(
    run: RunRow,
    primary: gpui::Hsla,
    border: gpui::Hsla,
    handle: Entity<Root>,
) -> impl IntoElement {
    let name = agent::find(&run.agent)
        .map(|agent| agent.name)
        .unwrap_or(run.agent.as_str());
    let run_id = run.id;
    let select = handle.clone();
    let terminal = handle.clone();
    let review = handle.clone();

    let elapsed = elapsed(&run);
    let mut body = div()
        .flex()
        .flex_col()
        .w(px(420.0))
        .max_w(relative(1.0))
        .gap_2()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(Text::new(SharedString::from(name.to_string())).bold())
                        .children(run.selected.then(|| {
                            Badge::new("selected")
                                .color(ColorName::Blue)
                                .variant(Variant::Light)
                        })),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .children(activity_chip(&run))
                        .child(status_badge(run.status))
                        .child(
                            Text::new(SharedString::from(elapsed))
                                .size(Size::Xs)
                                .dimmed(),
                        ),
                ),
        )
        // Lead with plain status language; the technical branch and worktree
        // drop to the secondary line below, keeping their explanatory tooltips.
        .child(Text::new(status_detail(run.status)).size(Size::Sm).dimmed())
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .id(SharedString::from(format!("branch-tip-{}", run.id)))
                        .tooltip(guise::tooltip(
                            "A branch is a named line of changes. This agent's work lands on its own branch so you can compare and merge it independently.",
                        ))
                        .child(
                            Text::new(SharedString::from(run.branch.clone()))
                                .size(Size::Xs)
                                .dimmed(),
                        ),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("worktree-tip-{}", run.id)))
                        .tooltip(guise::tooltip(
                            "A worktree is this agent's private copy of your project, so parallel agents never overwrite each other's files.",
                        ))
                        .child(
                            Text::new(SharedString::from(run.worktree.clone()))
                                .size(Size::Xs)
                                .dimmed(),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap_1()
                .child(Badge::new(format!("{} files", run.files)).variant(Variant::Light))
                .child(
                    Badge::new(format!("+{}", run.added))
                        .color(ColorName::Green)
                        .variant(Variant::Light),
                )
                .child(
                    Badge::new(format!("-{}", run.removed))
                        .color(ColorName::Red)
                        .variant(Variant::Light),
                )
                .child(Badge::new(format!("attempt {}", run.attempt)).variant(Variant::Light))
                .children(
                    run.exit_code
                        .map(|code| Badge::new(format!("exit {code}")).variant(Variant::Light)),
                )
                .children(run.checking.then(|| {
                    Badge::new("checks running")
                        .color(ColorName::Blue)
                        .variant(Variant::Light)
                }))
                .children((!run.checking && run.checks == 0).then(|| {
                    Badge::new("checks not run")
                        .color(ColorName::Gray)
                        .variant(Variant::Light)
                }))
                .children(
                    (!run.checking && run.check_status == Some(checks::Status::Pass)).then(|| {
                        Badge::new(format!("{} checks PASS", run.checks))
                            .color(ColorName::Green)
                            .variant(Variant::Light)
                    }),
                )
                .children(
                    (!run.checking && run.check_status == Some(checks::Status::Fail)).then(|| {
                        Badge::new(format!("{} checks FAIL", run.checks))
                            .color(ColorName::Red)
                            .variant(Variant::Light)
                    }),
                )
                .children(
                    (!run.checking && run.check_status == Some(checks::Status::Skipped)).then(
                        || {
                            Badge::new("checks skipped")
                                .color(ColorName::Gray)
                                .variant(Variant::Light)
                        },
                    ),
                ),
        );

    if let Some(error) = &run.error {
        body = body.child(
            Alert::new(SharedString::from(error.clone()))
                .title("Needs attention")
                .color(ColorName::Red),
        );
    }
    if let Some(term) = run.terminal.clone() {
        body = body.child(
            div()
                .w_full()
                .h(px(190.0))
                .overflow_hidden()
                .border_1()
                .rounded(px(4.0))
                .child(term),
        );
    } else if !run.output.trim().is_empty() {
        body = body.child(
            div()
                .h(px(120.0))
                .overflow_hidden()
                .p_2()
                .border_1()
                .rounded(px(4.0))
                .font_family("monospace")
                .text_size(px(11.0))
                .child(SharedString::from(output_tail(&run.output, 8))),
        );
    }

    let mut actions = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap_1()
        .child(
            Button::new(SharedString::from(format!("select-{run_id}")), "Select")
                .size(Size::Xs)
                .variant(Variant::Subtle)
                .on_click(move |_, _, cx| {
                    select.update(cx, |root, cx| {
                        root.select_run(run_id);
                        cx.notify();
                    });
                }),
        )
        .child(
            Button::new(
                SharedString::from(format!("terminal-{run_id}")),
                "Open terminal",
            )
            .size(Size::Xs)
            .variant(Variant::Subtle)
            .on_click(move |_, _, cx| {
                terminal.update(cx, |root, cx| {
                    root.open_run_terminal(run_id);
                    cx.notify();
                });
            }),
        )
        .child(
            Button::new(SharedString::from(format!("review-{run_id}")), "Review")
                .size(Size::Xs)
                .variant(Variant::Light)
                .on_click(move |_, window, cx| {
                    review.update(cx, |root, cx| {
                        root.select_run(run_id);
                        root.open_view(crate::state::View::Diff, window, cx);
                        cx.notify();
                    });
                }),
        );

    if matches!(run.status, RunStatus::Queued | RunStatus::Running) {
        let cancel = handle.clone();
        actions = actions.child(
            Button::new(SharedString::from(format!("cancel-{run_id}")), "Cancel")
                .size(Size::Xs)
                .variant(Variant::Subtle)
                .on_click(move |_, _, cx| {
                    cancel.update(cx, |root, cx| {
                        root.cancel_run(run_id, cx);
                        cx.notify();
                    });
                }),
        );
    } else if run.status.is_terminal() {
        let retry = handle.clone();
        let remove = handle.clone();
        actions = actions
            .child(
                Button::new(SharedString::from(format!("retry-{run_id}")), "Retry")
                    .size(Size::Xs)
                    .variant(Variant::Subtle)
                    .on_click(move |_, window, cx| {
                        retry.update(cx, |root, cx| {
                            root.retry_run(run_id, window, cx);
                            cx.notify();
                        });
                    }),
            )
            .child(
                Button::new(
                    SharedString::from(format!("remove-{run_id}")),
                    "Remove worktree",
                )
                .size(Size::Xs)
                .variant(Variant::Subtle)
                .on_click(move |_, _, cx| {
                    remove.update(cx, |root, cx| {
                        root.request_remove_worktree(run_id);
                        cx.notify();
                    });
                }),
            );
    }

    if run.status == RunStatus::Succeeded {
        let merge = handle.clone();
        let pr = handle;
        actions = actions
            .child(
                Button::new(
                    SharedString::from(format!("merge-{run_id}")),
                    "Merge winner",
                )
                .size(Size::Xs)
                .variant(Variant::Filled)
                .on_click(move |_, _, cx| {
                    merge.update(cx, |root, cx| {
                        root.request_merge(run_id);
                        cx.notify();
                    });
                }),
            )
            .child(
                Button::new(SharedString::from(format!("pr-{run_id}")), "Create PR")
                    .size(Size::Xs)
                    .variant(Variant::Subtle)
                    .on_click(move |_, _, cx| {
                        pr.update(cx, |root, cx| {
                            root.create_pr_for_run(run_id);
                            cx.notify();
                        });
                    }),
            );
    }
    body = body.child(actions);

    div()
        .id(SharedString::from(format!("run-{run_id}")))
        .w(px(420.0))
        .max_w(relative(1.0))
        .border_1()
        .border_color(if run.selected { primary } else { border })
        .rounded(px(6.0))
        .child(Card::new().padding(Size::Md).child(body))
}

pub(super) fn status_badge(status: RunStatus) -> impl IntoElement {
    let (label, color) = match status {
        RunStatus::Queued => ("queued", ColorName::Gray),
        RunStatus::Running => ("running", ColorName::Blue),
        RunStatus::Succeeded => ("succeeded", ColorName::Green),
        RunStatus::Failed => ("failed", ColorName::Red),
        RunStatus::Cancelled => ("cancelled", ColorName::Orange),
    };
    Badge::new(label).color(color).variant(Variant::Light)
}

fn status_detail(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Queued => "Waiting for an available run slot",
        RunStatus::Running => "Agent working",
        RunStatus::Succeeded => "Ready to review",
        RunStatus::Failed => "Open output, fix setup, or retry",
        RunStatus::Cancelled => "Worktree preserved for retry",
    }
}

fn elapsed(run: &RunRow) -> String {
    let Some(start) = run.started_at else {
        return "not started".into();
    };
    let end = run.ended_at.unwrap_or_else(crate::state::now);
    let seconds = end.saturating_sub(start);
    if seconds >= 60 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

fn output_tail(output: &str, lines: usize) -> String {
    let rows: Vec<&str> = output.lines().collect();
    rows[rows.len().saturating_sub(lines)..].join("\n")
}
