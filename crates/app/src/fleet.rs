//! The main content: the selected task's fan-out, one card per agent run.
//!
//! Each run card shows the agent, its branch, and a live status. In a later
//! phase the card body hosts an embedded terminal pane streaming the agent's
//! output; today it shows the run's branch and state so the shell reads true.

use gpui::prelude::*;
use gpui::{div, px, App, Entity, IntoElement, SharedString, Window};
use guise::prelude::*;

use crate::state::{Root, RunRow};
use store::RunStatus;

/// Build the main content from a render snapshot.
#[allow(clippy::too_many_arguments)]
pub fn main_content(
    project_name: String,
    task_title: Option<String>,
    runs: Vec<RunRow>,
    fanout: Vec<String>,
    compose: Entity<guise::TextInput>,
    handle: Entity<Root>,
    _window: &mut Window,
    _cx: &mut App,
) -> impl IntoElement {
    // The whole surface is a drop target: dropping files creates a task that
    // references them (drag-drop into the prompt).
    let drop_handle = handle.clone();
    let mut col = div()
        .id("fanout-drop")
        .flex()
        .flex_col()
        .w_full()
        .gap_4()
        .p(px(20.0))
        .on_drop::<gpui::ExternalPaths>(move |paths, _, cx| {
            let paths = paths.paths().to_vec();
            drop_handle.update(cx, |root, cx| {
                root.create_task_from_files(&paths);
                cx.notify();
            });
        });

    // The compose box: a new-task prompt over the fan-out agents.
    col = col.child(compose_box(project_name, &fanout, compose, handle.clone()));

    match task_title {
        Some(title) => {
            col = col.child(Divider::new());
            col = col.child(Title::new(SharedString::from(title)).order(3));
            if runs.is_empty() {
                let fan = handle.clone();
                col = col
                    .child(
                        Text::new("No runs yet — fan this task out to your agents.")
                            .size(Size::Sm)
                            .dimmed(),
                    )
                    .child(
                        Button::new("fanout-existing", "Fan out")
                            .size(Size::Sm)
                            .variant(Variant::Filled)
                            .on_click(move |_, _, cx| {
                                fan.update(cx, |root, cx| {
                                    root.run_fanout();
                                    cx.notify();
                                });
                            }),
                    );
            } else {
                let mut grid = div().flex().flex_row().flex_wrap().gap_4();
                for run in runs {
                    grid = grid.child(run_card(run, handle.clone()));
                }
                col = col.child(grid);
            }
        }
        None => {
            col = col.child(
                Text::new("Describe a task above and fan it out, or pick one from the tree.")
                    .size(Size::Sm)
                    .dimmed(),
            );
        }
    }

    col
}

/// The new-task compose box: a prompt input, the fan-out agent chips, and the
/// create / create-and-fan-out actions.
fn compose_box(
    project_name: String,
    fanout: &[String],
    compose: Entity<guise::TextInput>,
    handle: Entity<Root>,
) -> impl IntoElement {
    let create = handle.clone();
    let create_input = compose.clone();
    let fan = handle.clone();
    let fan_input = compose.clone();

    let mut agents = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_1()
        .child(Text::new("Agents:").size(Size::Xs).dimmed());
    for id in fanout {
        let name = agent::find(id).map(|a| a.name).unwrap_or(id.as_str());
        let icon = agent::find(id).map(|a| a.icon).unwrap_or("•");
        agents = agents
            .child(Badge::new(SharedString::from(format!("{icon} {name}"))).variant(Variant::Light));
    }

    Card::new().padding(Size::Md).child(
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Text::new(SharedString::from(format!("New task in {project_name}"))).bold())
            .child(compose)
            .child(agents)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        Button::new("create-task", "Create task")
                            .size(Size::Sm)
                            .variant(Variant::Subtle)
                            .on_click(move |_, _, cx| {
                                let prompt = create_input.read(cx).text();
                                create.update(cx, |root, cx| {
                                    root.create_task(&prompt, false);
                                    cx.notify();
                                });
                                create_input.update(cx, |i, cx| i.set_text("", cx));
                            }),
                    )
                    .child(
                        Button::new("create-fanout", "Create & fan out")
                            .size(Size::Sm)
                            .variant(Variant::Filled)
                            .on_click(move |_, _, cx| {
                                let prompt = fan_input.read(cx).text();
                                fan.update(cx, |root, cx| {
                                    root.create_task(&prompt, true);
                                    cx.notify();
                                });
                                fan_input.update(cx, |i, cx| i.set_text("", cx));
                            }),
                    ),
            ),
    )
}

/// One agent's run, as a card.
fn run_card(run: RunRow, handle: Entity<Root>) -> impl IntoElement {
    let name = agent::find(&run.agent).map(|a| a.name).unwrap_or(run.agent.as_str());
    let icon = agent::find(&run.agent).map(|a| a.icon).unwrap_or("•");
    let can_merge = run.status == RunStatus::Succeeded;
    let run_id = run.id;

    let mut body = div()
        .flex()
        .flex_col()
        .w(px(280.0))
        .gap_2()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(Text::new(SharedString::from(format!("{icon}  {name}"))).bold())
                .child(status_badge(run.status)),
        )
        .child(
            Text::new(SharedString::from(run.branch.clone()))
                .size(Size::Xs)
                .dimmed(),
        )
        .child(Divider::new())
        .child(Text::new(SharedString::from(status_detail(run.status))).size(Size::Sm));

    // A succeeded run can be merged as the winner or opened as a PR.
    if can_merge {
        let merge_handle = handle.clone();
        let pr_handle = handle.clone();
        body = body.child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(
                    Button::new(SharedString::from(format!("merge-{run_id}")), "Merge winner")
                        .size(Size::Xs)
                        .variant(Variant::Light)
                        .on_click(move |_, _, cx| {
                            merge_handle.update(cx, |root, cx| {
                                root.merge_run(run_id);
                                cx.notify();
                            });
                        }),
                )
                .child(
                    Button::new(SharedString::from(format!("pr-{run_id}")), "Create PR")
                        .size(Size::Xs)
                        .variant(Variant::Subtle)
                        .on_click(move |_, _, cx| {
                            pr_handle.update(cx, |root, cx| {
                                root.create_pr_for_run(run_id);
                                cx.notify();
                            });
                        }),
                ),
        );
    }

    div()
        .id(SharedString::from(format!("run-{run_id}")))
        .child(Card::new().padding(Size::Md).child(body))
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
        RunStatus::Queued => "Worktree ready · agent not started",
        RunStatus::Running => "Agent working…",
        RunStatus::Succeeded => "Finished cleanly · ready to review",
        RunStatus::Failed => "Agent exited non-zero",
        RunStatus::Cancelled => "Cancelled by you",
    }
}
