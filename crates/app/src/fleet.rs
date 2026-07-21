//! Task composition, run comparison, and run terminal surfaces.

mod cards;
mod composer;

use gpui::prelude::*;
use gpui::{div, px, App, Entity, IntoElement, SharedString, Window};
use guise::prelude::*;

use cards::{run_card, status_badge};
use composer::compose_box;

use crate::control::Button;
use crate::state::{Root, RunRow};
use store::{RunStatus, TaskStatus};

#[allow(clippy::too_many_arguments)]
pub fn main_content(
    project_name: String,
    task_title: Option<String>,
    task_status: Option<TaskStatus>,
    task_id: Option<i64>,
    runs: Vec<RunRow>,
    fanout: Vec<String>,
    reports: Vec<(agent::registry::Agent, agent::doctor::Report)>,
    advanced: bool,
    show_all: bool,
    preparing: bool,
    setup_checks: Vec<crate::setup::Check>,
    setup_open: bool,
    layout_names: Vec<String>,
    compose: Entity<guise::TextInput>,
    start_ref: Entity<guise::TextInput>,
    handle: Entity<Root>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let setup_blocked = setup_checks
        .iter()
        .any(|check| check.status == crate::setup::Status::Blocked);
    let setup_attention = setup_checks
        .iter()
        .any(|check| check.status == crate::setup::Status::Attention);
    let can_run = reports
        .iter()
        .any(|(agent, report)| fanout.contains(&agent.id) && report.ready());
    let (run_primary, run_border) = {
        let t = guise::theme::theme(cx);
        (t.primary().hsla(), t.border().hsla())
    };
    let drop_handle = handle.clone();
    let mut col = div()
        .id("fanout-drop")
        .flex()
        .flex_col()
        .w_full()
        .gap_4()
        .p(px(20.0))
        .overflow_y_scroll()
        .on_drop::<gpui::ExternalPaths>(move |paths, _, cx| {
            let paths = paths.paths().to_vec();
            drop_handle.update(cx, |root, cx| {
                root.create_task_from_files(&paths);
                cx.notify();
            });
        });

    // The workflow stepper only has stages worth showing once a task exists.
    if task_title.is_some() {
        col = col.child(workflow(task_title.is_some(), task_status, &runs, cx));
    }
    // The setup doctor earns the composer's prime space only when it needs the
    // user - something blocked or awaiting verification - or they opened it. Its
    // all-clear state stays out of the way.
    if setup_blocked || setup_attention || setup_open {
        col = col.child(crate::setup::panel(
            setup_checks,
            setup_open,
            handle.clone(),
        ));
    }
    col = col.child(compose_box(
        project_name,
        &fanout,
        reports,
        advanced,
        show_all,
        preparing,
        setup_blocked,
        layout_names,
        compose,
        start_ref,
        handle.clone(),
    ));

    match task_title {
        Some(title) => {
            col = col.child(Divider::new());
            col = col.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(Title::new(SharedString::from(title)).order(3))
                    .child(next_action(task_status, &runs)),
            );
            if runs.is_empty() {
                let fan = handle.clone();
                col = col.child(
                    Alert::new("The task is drafted. Choose agents above, then start the run.")
                        .title("Ready to dispatch")
                        .color(ColorName::Blue),
                );
                col = col.child(
                    Button::new("fanout-existing", "Start selected agents")
                        .size(Size::Sm)
                        .variant(Variant::Filled)
                        .disabled(!can_run || preparing || setup_blocked)
                        .on_click(move |_, window, cx| {
                            fan.update(cx, |root, cx| {
                                root.run_fanout(window, cx);
                                cx.notify();
                            });
                        }),
                );
            } else {
                let mut grid = div().flex().flex_row().flex_wrap().items_start().gap_4();
                for run in runs.clone() {
                    grid = grid.child(run_card(run, run_primary, run_border, handle.clone()));
                }
                col = col.child(grid);
                if runs.iter().all(|run| run.status.is_terminal()) {
                    if let Some(task_id) = task_id {
                        let cleanup = handle.clone();
                        col = col.child(
                            Button::new("cleanup-task", "Clean up finished worktrees")
                                .size(Size::Xs)
                                .variant(Variant::Subtle)
                                .on_click(move |_, _, cx| {
                                    cleanup.update(cx, |root, cx| {
                                        root.confirm =
                                            Some(crate::run::ConfirmAction::CleanupTask(task_id));
                                        cx.notify();
                                    });
                                }),
                        );
                    }
                }
            }
        }
        None => {
            let test = handle.clone();
            let settings = handle;
            col = col.child(
                Alert::new("Choose a template or describe one concrete outcome. You can start with one agent and add more when comparison is useful.")
                    .title("Create the first task")
                    .color(ColorName::Blue),
            );
            col = col.child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        div()
                            .id("first-test-tip")
                            .tooltip(guise::tooltip("Asks one selected agent to inspect the repository without requesting code changes."))
                            .child(
                                Button::new("first-test", "Run a setup test")
                                    .size(Size::Sm)
                                    .variant(Variant::Filled)
                                    .disabled(!can_run || setup_blocked)
                                    .on_click(move |_, window, cx| {
                                        test.update(cx, |root, cx| {
                                            root.create_task(
                                                "Inspect this repository. Summarize what it builds, identify its primary verification command, and do not change files.",
                                                true,
                                                window,
                                                cx,
                                            );
                                            cx.notify();
                                        });
                                    }),
                            ),
                    )
                    .child(
                        Button::new("first-settings", "Agent settings")
                            .size(Size::Sm)
                            .variant(Variant::Subtle)
                            .on_click(move |_, window, cx| {
                                settings.update(cx, |root, cx| {
                                    root.open_view(crate::state::View::Settings, window, cx);
                                    cx.notify();
                                });
                            }),
                    ),
            );
        }
    }

    let _ = (window, cx);
    col
}

fn workflow(
    has_task: bool,
    task_status: Option<TaskStatus>,
    runs: &[RunRow],
    cx: &App,
) -> impl IntoElement {
    let active = if !has_task {
        1
    } else if runs.is_empty() {
        2
    } else if runs
        .iter()
        .any(|run| matches!(run.status, RunStatus::Queued | RunStatus::Running))
    {
        3
    } else if task_status == Some(TaskStatus::Merged) {
        5
    } else {
        4
    };
    let theme = guise::theme::theme(cx);
    let done = theme.primary().hsla();
    let idle = theme.border().hsla();
    let mut row = div().flex().flex_row().items_center().w_full();
    for (index, label, tip) in [
        (
            1,
            "Setup",
            "Open a repository and verify at least one agent is ready.",
        ),
        (
            2,
            "Task",
            "Describe one testable outcome and select the agents to try it.",
        ),
        (3, "Run", "Each agent works in an isolated git worktree."),
        (
            4,
            "Review",
            "Compare changes, checks, and terminal output before choosing.",
        ),
        (
            5,
            "Merge",
            "Merge the winner or open a pull request, then clean up.",
        ),
    ] {
        if index > 1 {
            row =
                row.child(
                    div()
                        .h(px(1.0))
                        .flex_1()
                        .bg(if active >= index { done } else { idle }),
                );
        }
        row = row.child(
            div()
                .id(SharedString::from(format!("workflow-{index}")))
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .tooltip(guise::tooltip(tip))
                .child(
                    div()
                        .size(px(22.0))
                        .rounded(px(11.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(if active >= index { done } else { idle })
                        .text_size(px(11.0))
                        .child(index.to_string()),
                )
                .child(Text::new(label).size(Size::Xs).dimmed()),
        );
    }
    row
}

pub fn run_terminal(
    run_id: i64,
    root: &Root,
    handle: Entity<Root>,
    _window: &mut Window,
    _cx: &mut App,
) -> impl IntoElement {
    let mut col = div().flex().flex_col().size_full().gap_2().p_2();
    let Some(run) = root.db.run(run_id).ok() else {
        return col.child(Alert::new("This run no longer exists.").color(ColorName::Red));
    };
    col = col.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(Text::new(SharedString::from(run.branch.clone())).bold())
            .child(status_badge(run.status)),
    );
    if let Some(term) = root.run_terms.get(&run_id) {
        col = col.child(div().flex_1().overflow_hidden().child(term.clone()));
    } else {
        col = col.child(
            div()
                .id(SharedString::from(format!("run-output-{run_id}")))
                .flex_1()
                .overflow_y_scroll()
                .p_3()
                .border_1()
                .rounded(px(4.0))
                .font_family("monospace")
                .text_size(px(12.0))
                .child(SharedString::from(if run.output.is_empty() {
                    "No terminal output was captured.".into()
                } else {
                    run.output
                })),
        );
    }
    if run.status.is_terminal() {
        let retry = handle;
        col = col.child(
            Button::new(
                SharedString::from(format!("terminal-retry-{run_id}")),
                "Retry in this worktree",
            )
            .size(Size::Xs)
            .variant(Variant::Filled)
            .on_click(move |_, window, cx| {
                retry.update(cx, |root, cx| {
                    root.retry_run(run_id, window, cx);
                    cx.notify();
                });
            }),
        );
    }
    col
}

fn next_action(status: Option<TaskStatus>, runs: &[RunRow]) -> impl IntoElement {
    let (label, color) = if status == Some(TaskStatus::Merged) {
        ("Merged", ColorName::Green)
    } else if runs.is_empty() {
        ("Next: run", ColorName::Blue)
    } else if runs.iter().any(|run| run.status == RunStatus::Running) {
        ("Agents working", ColorName::Blue)
    } else if runs.iter().any(|run| run.status == RunStatus::Queued) {
        ("Queued", ColorName::Gray)
    } else {
        ("Next: review", ColorName::Green)
    };
    Badge::new(label).color(color).variant(Variant::Light)
}
