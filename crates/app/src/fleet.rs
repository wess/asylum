//! Task composition, run comparison, and run terminal surfaces.

use gpui::prelude::*;
use gpui::{div, px, relative, App, Entity, IntoElement, SharedString, Window};
use guise::prelude::*;

use crate::control::{Button, Switch};
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
    let can_run = reports
        .iter()
        .any(|(agent, report)| fanout.contains(&agent.id) && report.ready());
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

    col = col.child(workflow(task_title.is_some(), task_status, &runs, cx));
    col = col.child(crate::setup::panel(
        setup_checks,
        setup_open,
        handle.clone(),
    ));
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
                    grid = grid.child(run_card(run, handle.clone()));
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

#[allow(clippy::too_many_arguments)]
fn compose_box(
    project_name: String,
    fanout: &[String],
    reports: Vec<(agent::registry::Agent, agent::doctor::Report)>,
    advanced: bool,
    show_all: bool,
    preparing: bool,
    setup_blocked: bool,
    layouts: Vec<String>,
    compose: Entity<guise::TextInput>,
    start_ref: Entity<guise::TextInput>,
    handle: Entity<Root>,
) -> impl IntoElement {
    let create = handle.clone();
    let create_input = compose.clone();
    let fan = handle.clone();
    let fan_input = compose.clone();
    let toggle = handle.clone();

    let ready = reports.iter().filter(|(_, report)| report.ready()).count();
    let selected_ready = reports
        .iter()
        .filter(|(agent, report)| fanout.contains(&agent.id) && report.ready())
        .count();
    let mut selected = div().flex().flex_row().flex_wrap().items_center().gap_1();
    for id in fanout {
        let report = reports.iter().find(|(agent, _)| &agent.id == id);
        let (label, color) = match report {
            Some((agent, report)) if report.verified() => {
                (format!("{} verified", agent.name), ColorName::Green)
            }
            Some((agent, report)) if report.ready() => {
                (format!("{} installed", agent.name), ColorName::Orange)
            }
            Some((agent, _)) => (format!("{} missing", agent.name), ColorName::Red),
            None => (id.clone(), ColorName::Red),
        };
        selected = selected.child(
            Badge::new(SharedString::from(label))
                .color(color)
                .variant(Variant::Light),
        );
    }
    if fanout.is_empty() {
        selected = selected.child(
            Badge::new("No agents selected")
                .color(ColorName::Red)
                .variant(Variant::Light),
        );
    }

    let mut body = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(Text::new(SharedString::from(format!("New task in {project_name}"))).bold())
                .child(
                    Button::new(
                        "composer-advanced",
                        if advanced {
                            "Hide agent controls"
                        } else {
                            "Choose agents"
                        },
                    )
                    .size(Size::Xs)
                    .variant(Variant::Subtle)
                    .on_click(move |_, _, cx| {
                        toggle.update(cx, |root, cx| {
                            root.composer_advanced = !root.composer_advanced;
                            cx.notify();
                        });
                    }),
                ),
        )
        .child(template_row(compose.clone()))
        .child(compose)
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(selected)
                .child(
                    Text::new(SharedString::from(format!("{ready} installed")))
                        .size(Size::Xs)
                        .dimmed(),
                ),
        );

    if advanced {
        if !layouts.is_empty() {
            let mut row = div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(Text::new("Layouts").size(Size::Xs).dimmed());
            for name in &layouts {
                let pick = handle.clone();
                let chosen = name.clone();
                row = row.child(
                    Button::new(SharedString::from(format!("layout-{name}")), name.clone())
                        .size(Size::Xs)
                        .variant(Variant::Subtle)
                        .on_click(move |_, _, cx| {
                            pick.update(cx, |root, cx| {
                                root.apply_layout(&chosen);
                                cx.notify();
                            });
                        }),
                );
            }
            body = body.child(row);
        }
        body = body.child(agent_controls(reports, fanout, show_all, handle.clone()));
        body = body.child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(Text::new("Start worktrees from").size(Size::Xs).dimmed())
                .child(start_ref),
        );
    }

    body = body.child(
        div()
            .flex()
            .flex_row()
            .gap_2()
            .child(
                Button::new("create-task", "Save draft")
                    .size(Size::Sm)
                    .variant(Variant::Subtle)
                    .on_click(move |_, window, cx| {
                        let prompt = create_input.read(cx).text();
                        create.update(cx, |root, cx| {
                            root.create_task(&prompt, false, window, cx);
                            cx.notify();
                        });
                        if !prompt.trim().is_empty() {
                            create_input.update(cx, |input, cx| input.set_text("", cx));
                        }
                    }),
            )
            .child(
                Button::new("create-fanout", "Create and run")
                    .size(Size::Sm)
                    .variant(Variant::Filled)
                    .disabled(selected_ready == 0 || preparing || setup_blocked)
                    .on_click(move |_, window, cx| {
                        let prompt = fan_input.read(cx).text();
                        fan.update(cx, |root, cx| {
                            root.create_task(&prompt, true, window, cx);
                            cx.notify();
                        });
                        if !prompt.trim().is_empty() {
                            fan_input.update(cx, |input, cx| input.set_text("", cx));
                        }
                    }),
            ),
    );

    if preparing {
        body = body.child(
            Alert::new("Creating isolated worktrees and running project setup commands.")
                .title("Preparing runs")
                .color(ColorName::Blue),
        );
    } else if setup_blocked {
        body = body.child(
            Alert::new(
                "Resolve blocked setup items before starting agents. Drafts remain available.",
            )
            .title("Setup required")
            .color(ColorName::Red),
        );
    }

    Card::new().padding(Size::Md).child(body)
}

fn template_row(compose: Entity<guise::TextInput>) -> impl IntoElement {
    let mut row = div().flex().flex_row().flex_wrap().gap_1();
    for (index, label, prompt) in [
        (
            0,
            "Fix bug",
            "Fix this bug:\n\nExpected:\nActual:\nRelevant files:",
        ),
        (
            1,
            "Build feature",
            "Build this feature:\n\nOutcome:\nConstraints:\nAcceptance checks:",
        ),
        (
            2,
            "Add tests",
            "Add test coverage for:\n\nCases to cover:\nExpected behavior:",
        ),
        (
            3,
            "Refactor",
            "Refactor this code without changing behavior:\n\nTarget:\nConstraints:\nValidation:",
        ),
        (
            4,
            "Review",
            "Review this code for correctness, regressions, and missing tests:\n\nScope:",
        ),
        (
            5,
            "Design",
            "Implement this design change:\n\nUser outcome:\nReference:\nResponsive behavior:",
        ),
    ] {
        let input = compose.clone();
        row = row.child(
            Button::new(SharedString::from(format!("template-{index}")), label)
                .size(Size::Xs)
                .variant(Variant::Light)
                .on_click(move |_, _, cx| input.update(cx, |input, cx| input.set_text(prompt, cx))),
        );
    }
    row
}

fn agent_controls(
    reports: Vec<(agent::registry::Agent, agent::doctor::Report)>,
    fanout: &[String],
    show_all: bool,
    handle: Entity<Root>,
) -> impl IntoElement {
    let mut panel = div()
        .flex()
        .flex_col()
        .gap_1()
        .p_2()
        .border_1()
        .rounded(px(6.0));
    for (agent, report) in reports
        .into_iter()
        .filter(|(agent, report)| show_all || report.ready() || fanout.contains(&agent.id))
    {
        let id = agent.id.clone();
        let selected = fanout.contains(&id);
        let ready = report.ready();
        let verified = report.verified();
        let toggle = handle.clone();
        panel = panel.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .py_1()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(Text::new(SharedString::from(agent.name.clone())).size(Size::Sm))
                        .child(
                            Badge::new(if verified {
                                "verified"
                            } else if ready {
                                "installed"
                            } else {
                                "not installed"
                            })
                            .color(if verified {
                                ColorName::Green
                            } else if ready {
                                ColorName::Orange
                            } else {
                                ColorName::Red
                            })
                            .variant(Variant::Light),
                        ),
                )
                .child(
                    Switch::new(SharedString::from(format!("pick-{id}")))
                        .checked(selected)
                        .aria_label(SharedString::from(format!("Select {}", agent.name)))
                        .disabled(!ready)
                        .size(Size::Sm)
                        .on_change(move |_, _, cx| {
                            toggle.update(cx, |root, cx| {
                                root.toggle_agent(&id);
                                cx.notify();
                            });
                        }),
                ),
        );
    }
    let show = handle;
    panel.child(
        Button::new(
            "show-all-agents",
            if show_all {
                "Show ready agents"
            } else {
                "Show full catalog"
            },
        )
        .size(Size::Xs)
        .variant(Variant::Subtle)
        .on_click(move |_, _, cx| {
            show.update(cx, |root, cx| {
                root.show_all_agents = !root.show_all_agents;
                cx.notify();
            });
        }),
    )
}

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

fn run_card(run: RunRow, handle: Entity<Root>) -> impl IntoElement {
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
                        .child(status_badge(run.status)),
                ),
        )
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
                .child(Badge::new(elapsed).variant(Variant::Light))
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
    } else {
        body = body.child(Text::new(status_detail(run.status)).size(Size::Sm).dimmed());
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
        .border_color(if run.selected {
            gpui::rgb(0x3b82f6)
        } else {
            gpui::rgba(0x88888855)
        })
        .rounded(px(6.0))
        .child(Card::new().padding(Size::Md).child(body))
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

fn status_badge(status: RunStatus) -> impl IntoElement {
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
