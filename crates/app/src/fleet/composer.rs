//! The task composer: templates, agent selection, layouts, and dispatch.

use gpui::prelude::*;
use gpui::{div, px, Entity, IntoElement, SharedString};
use guise::prelude::*;

use crate::control::{Button, Switch};
use crate::state::Root;

#[allow(clippy::too_many_arguments)]
pub(super) fn compose_box(
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
                            "Fewer options"
                        } else {
                            "More options"
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
        .child(ready_chips(&reports, fanout, handle.clone()))
        .child(layout_presets(&layouts, ready, handle.clone()));

    if advanced {
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

    // The primary action never dead-ends: when disabled, it carries the
    // computed reason as a tooltip.
    let run_reason = run_disabled_reason(selected_ready, preparing, setup_blocked);
    let run_button = Button::new("create-fanout", "Create and run")
        .size(Size::Sm)
        .variant(Variant::Filled)
        .disabled(run_reason.is_some())
        .on_click(move |_, window, cx| {
            let prompt = fan_input.read(cx).text();
            fan.update(cx, |root, cx| {
                root.create_task(&prompt, true, window, cx);
                cx.notify();
            });
            if !prompt.trim().is_empty() {
                fan_input.update(cx, |input, cx| input.set_text("", cx));
            }
        });
    let run_control = match run_reason {
        Some(reason) => div()
            .id("create-fanout-tip")
            .tooltip(guise::tooltip(reason))
            .child(run_button)
            .into_any_element(),
        None => run_button.into_any_element(),
    };

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
            .child(run_control),
    );

    if preparing {
        let cancel = handle.clone();
        body = body.child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    Alert::new("Creating isolated worktrees and running project setup commands.")
                        .title("Preparing runs")
                        .color(ColorName::Blue),
                )
                .child(
                    Button::new("cancel-fanout", "Cancel setup")
                        .size(Size::Xs)
                        .variant(Variant::Subtle)
                        .on_click(move |_, _, cx| {
                            cancel.update(cx, |root, cx| {
                                root.cancel_fanout();
                                cx.notify();
                            });
                        }),
                ),
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

/// Why the "Create and run" button is disabled, if it is - surfaced as its
/// tooltip so the control is never a dead end. `None` means it is enabled.
fn run_disabled_reason(
    selected_ready: usize,
    preparing: bool,
    setup_blocked: bool,
) -> Option<&'static str> {
    if preparing {
        Some("Preparing worktrees and running project setup. Agents start automatically.")
    } else if setup_blocked {
        Some("Resolve the blocked setup items above before starting agents.")
    } else if selected_ready == 0 {
        Some("Select at least one installed agent to run.")
    } else {
        None
    }
}

/// The ready agents as always-visible toggle chips - the app's core "fan one
/// prompt across competing agents" choice, no longer hidden behind a text
/// toggle. Selected chips are tinted (green verified / orange installed) and
/// carry a check; a selected-but-uninstalled agent keeps its missing badge so
/// the warning is never lost.
fn ready_chips(
    reports: &[(agent::registry::Agent, agent::doctor::Report)],
    fanout: &[String],
    handle: Entity<Root>,
) -> impl IntoElement {
    let mut row = div().flex().flex_row().flex_wrap().items_center().gap_1();
    let mut any_ready = false;
    for (agent, report) in reports.iter().filter(|(_, report)| report.ready()) {
        any_ready = true;
        let id = agent.id.clone();
        let selected = fanout.contains(&id);
        let color = if report.verified() {
            ColorName::Green
        } else {
            ColorName::Orange
        };
        let toggle = handle.clone();
        row = row.child(
            Chip::new(
                SharedString::from(format!("chip-{id}")),
                SharedString::from(agent.name.clone()),
            )
            .checked(selected)
            .color(color)
            .size(Size::Sm)
            .on_change(move |_, _, cx| {
                toggle.update(cx, |root, cx| {
                    root.toggle_agent(&id);
                    cx.notify();
                });
            }),
        );
    }
    for id in fanout {
        if reports
            .iter()
            .any(|(agent, report)| &agent.id == id && report.ready())
        {
            continue;
        }
        let label = reports
            .iter()
            .find(|(agent, _)| &agent.id == id)
            .map(|(agent, _)| format!("{} missing", agent.name))
            .unwrap_or_else(|| format!("{id} missing"));
        row = row.child(
            Badge::new(SharedString::from(label))
                .color(ColorName::Red)
                .variant(Variant::Light),
        );
    }
    if !any_ready {
        row = row.child(
            Text::new("No installed agents detected yet.")
                .size(Size::Xs)
                .dimmed(),
        );
    }
    row
}

/// One-click fan-out presets (duel/triad/swarm) that set the whole selection,
/// promoted out of the advanced disclosure. The installed count rides along on
/// the right as a quick reference.
fn layout_presets(layouts: &[String], ready: usize, handle: Entity<Root>) -> impl IntoElement {
    let mut left = div().flex().flex_row().flex_wrap().items_center().gap_2();
    if !layouts.is_empty() {
        left = left.child(Text::new("Layouts").size(Size::Xs).dimmed());
        for name in layouts {
            let pick = handle.clone();
            let chosen = name.clone();
            left = left.child(
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
    }
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_2()
        .child(left)
        .child(
            Text::new(SharedString::from(format!("{ready} installed")))
                .size(Size::Xs)
                .dimmed(),
        )
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
        let install = report.install;
        let toggle = handle.clone();
        let switch = Switch::new(SharedString::from(format!("pick-{id}")))
            .checked(selected)
            .aria_label(SharedString::from(format!("Select {}", agent.name)))
            .disabled(!ready)
            .size(Size::Sm)
            .on_change(move |_, _, cx| {
                toggle.update(cx, |root, cx| {
                    root.toggle_agent(&id);
                    cx.notify();
                });
            });
        // A disabled switch never dead-ends: explain why in a tooltip, with
        // the install command when one is known.
        let switch_control = if ready {
            switch.into_any_element()
        } else {
            let reason = match install {
                Some(hint) => format!("{} is not installed. Install: {hint}", agent.name),
                None => format!("{} is not installed on PATH.", agent.name),
            };
            div()
                .id(SharedString::from(format!("pick-tip-{}", agent.id)))
                .tooltip(guise::tooltip(reason))
                .child(switch)
                .into_any_element()
        };
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
                .child(switch_control),
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
