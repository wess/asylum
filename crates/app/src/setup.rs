//! First-run and project readiness checks shown before task dispatch.

use std::path::Path;

use gpui::prelude::*;
use gpui::{div, px, ClipboardItem, Entity, IntoElement, SharedString};
use guise::prelude::*;

use crate::control::Button;
use crate::state::Root;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pass,
    Attention,
    Blocked,
}

#[derive(Debug, Clone)]
pub struct Check {
    pub label: String,
    pub detail: String,
    pub status: Status,
    /// Copy-pasteable install commands surfaced alongside this check
    /// (agent name, install command), each rendered with a one-click copy
    /// button rather than left for the user to select and copy by hand.
    pub hints: Vec<(String, String)>,
    /// A decision this check is asking the user to make, rendered as a button
    /// beside it. A check that reports a permission the user has not granted is
    /// useless without the means to grant it.
    pub action: Option<CheckAction>,
}

/// A decision offered directly from a readiness check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckAction {
    /// Trust this repository to run the commands it declares for itself.
    TrustProject(i64),
    /// Withdraw that trust.
    RevokeProjectTrust(i64),
}

impl Check {
    fn new(label: impl Into<String>, detail: impl Into<String>, status: Status) -> Self {
        Self {
            label: label.into(),
            detail: detail.into(),
            status,
            hints: Vec::new(),
            action: None,
        }
    }

    fn with_hints(mut self, hints: Vec<(String, String)>) -> Self {
        self.hints = hints;
        self
    }

    fn with_action(mut self, action: CheckAction) -> Self {
        self.action = Some(action);
        self
    }
}

pub fn inspect(root: &Root) -> Vec<Check> {
    let mut checks = Vec::new();
    let Some(project_id) = root.project_id else {
        return checks;
    };
    let Ok(project) = root.db.project(project_id) else {
        return vec![Check::new(
            "Project",
            "The selected project could not be loaded from the workspace store.",
            Status::Blocked,
        )];
    };
    let path = Path::new(&project.path);

    checks.push(match agent::doctor::find_program("git") {
        Some(_) => Check::new("Git", "Available on PATH", Status::Pass),
        None => Check::new(
            "Git",
            "Git is not on PATH. Worktrees and merges cannot run.",
            Status::Blocked,
        ),
    });
    checks.push(if git::is_repo(path) {
        Check::new("Repository", "Valid git worktree", Status::Pass)
    } else {
        Check::new(
            "Repository",
            "The project path is no longer a valid git worktree.",
            Status::Blocked,
        )
    });

    let (project_config, diagnostics) = config::load_project(path);
    let base = project_config
        .base_branch
        .clone()
        .unwrap_or(project.base_branch.clone());
    let base_exists = git::branch::branches(path)
        .map(|branches| branches.iter().any(|branch| branch.name == base))
        .unwrap_or(false);
    checks.push(if base_exists {
        Check::new("Base branch", base, Status::Pass)
    } else {
        Check::new(
            "Base branch",
            format!("{base} was not found. Correct the project base branch before dispatch."),
            Status::Blocked,
        )
    });

    checks.push(match git::worktree::list(path) {
        Ok(worktrees) => Check::new(
            "Worktrees",
            format!("Supported; {} currently registered", worktrees.len()),
            Status::Pass,
        ),
        Err(error) => Check::new(
            "Worktrees",
            format!("Git worktree inspection failed: {error}"),
            Status::Blocked,
        ),
    });

    let reports = root.agent_reports();
    let installed = reports.iter().filter(|(_, report)| report.ready()).count();
    let verified = reports
        .iter()
        .filter(|(_, report)| report.verified())
        .count();
    checks.push(if verified > 0 {
        Check::new(
            "Agents",
            format!("{verified} verified; {installed} installed"),
            Status::Pass,
        )
    } else if installed > 0 {
        Check::new(
            "Agents",
            format!("{installed} installed; run the setup test to verify authentication"),
            Status::Attention,
        )
    } else {
        // Nothing installed: show copy-pasteable install lines, each with a
        // one-click copy button. Prefer the agents the user chose as
        // defaults, else a few well-known ones.
        let preferred = &root.settings.default_agents;
        let hints: Vec<(String, String)> = reports
            .iter()
            .filter(|(agent, _)| preferred.is_empty() || preferred.contains(&agent.id))
            .filter_map(|(agent, report)| {
                report.install.map(|hint| (agent.name.clone(), hint.to_string()))
            })
            .take(3)
            .collect();
        let detail = if hints.is_empty() {
            "No agent CLI was found on PATH. Install one (e.g. Claude Code) and enable it in Settings.".to_string()
        } else {
            "No agent CLI was found on PATH. Install one, then reopen the project:".to_string()
        };
        Check::new("Agents", detail, Status::Blocked).with_hints(hints)
    });

    let detected_checks = checks::detect(path);
    if detected_checks.iter().any(|check| check.program == "bun") {
        checks.push(match agent::doctor::find_program("bun") {
            Some(_) => Check::new("Bun", "Available for project checks", Status::Pass),
            None => Check::new(
                "Bun",
                "A declared package check needs Bun, but Bun is not on PATH.",
                Status::Blocked,
            ),
        });
    }
    if detected_checks.iter().any(|check| check.program == "cargo") {
        checks.push(match agent::doctor::find_program("cargo") {
            Some(_) => Check::new("Cargo", "Available for project checks", Status::Pass),
            None => Check::new(
                "Cargo",
                "Cargo.toml was found, but Cargo is not on PATH.",
                Status::Blocked,
            ),
        });
    }
    for diagnostic in diagnostics {
        checks.push(Check::new(
            "Project configuration",
            diagnostic.message,
            Status::Attention,
        ));
    }
    // Repository trust. `asylum.toml` is written by whoever authored the repo,
    // and its `setup`/`env` are the two fields that execute, so what this check
    // reports depends entirely on whether the user has granted permission.
    let trusted = project.trusted();
    if project_config.declares_execution() {
        let commands = project_config.setup.len();
        let vars = project_config.env.len();
        if trusted {
            checks.push(
                Check::new(
                    "Worktree setup",
                    match (commands, vars) {
                        (0, v) => format!("{v} environment override(s) will be set for each agent"),
                        (c, 0) => format!("{c} setup command(s) will run before each agent starts"),
                        (c, v) => format!(
                            "{c} setup command(s) and {v} environment override(s) will be applied"
                        ),
                    },
                    Status::Attention,
                )
                .with_action(CheckAction::RevokeProjectTrust(project.id)),
            );
        } else {
            checks.push(
                Check::new(
                    "Repository not trusted",
                    match (commands, vars) {
                        (0, v) => format!(
                            "This repository sets {v} environment override(s) for agents. \
                             They will not be applied until you trust it."
                        ),
                        (c, 0) => format!(
                            "This repository declares {c} setup command(s). \
                             They will not run until you trust it."
                        ),
                        (c, v) => format!(
                            "This repository declares {c} setup command(s) and {v} environment \
                             override(s). Neither will be applied until you trust it."
                        ),
                    },
                    Status::Attention,
                )
                .with_action(CheckAction::TrustProject(project.id)),
            );
        }
    } else if trusted {
        checks.push(
            Check::new(
                "Repository trusted",
                "It declares nothing that runs today; anything it adds later will be applied.",
                Status::Attention,
            )
            .with_action(CheckAction::RevokeProjectTrust(project.id)),
        );
    }
    for diagnostic in &root.settings_diagnostics {
        checks.push(Check::new(
            "Settings",
            diagnostic.message.clone(),
            Status::Attention,
        ));
    }
    checks
}

impl Root {
    pub fn refresh_setup(&mut self) {
        self.setup_checks = inspect(self);
    }
}

pub fn panel(checks: Vec<Check>, open: bool, handle: Entity<Root>) -> impl IntoElement {
    let blocked = checks
        .iter()
        .filter(|check| check.status == Status::Blocked)
        .count();
    let attention = checks
        .iter()
        .filter(|check| check.status == Status::Attention)
        .count();
    let toggle = handle.clone();
    if !open {
        return div()
            .child(
                Button::new(
                    "show-setup",
                    SharedString::from(if blocked > 0 {
                        format!("Review {blocked} setup issue(s)")
                    } else if attention > 0 {
                        format!("Review {attention} setup item(s)")
                    } else {
                        "Project setup is ready".to_string()
                    }),
                )
                .size(Size::Xs)
                .variant(Variant::Subtle)
                .on_click(move |_, _, cx| {
                    toggle.update(cx, |root, cx| {
                        root.setup_open = true;
                        cx.notify();
                    });
                }),
            )
            .into_any_element();
    }

    let settings = handle.clone();
    let actions = handle.clone();
    let hide = handle;
    let mut rows = div().flex().flex_col().gap_1();
    for (index, check) in checks.into_iter().enumerate() {
        let Check {
            label,
            detail,
            status,
            hints,
            action,
        } = check;
        let (status_label, color) = match status {
            Status::Pass => ("ready", ColorName::Green),
            Status::Attention => ("verify", ColorName::Orange),
            Status::Blocked => ("blocked", ColorName::Red),
        };
        let mut left = div()
            .flex()
            .flex_col()
            .min_w(px(0.0))
            .flex_1()
            .child(Text::new(SharedString::from(label)).size(Size::Sm))
            .child(
                Text::new(SharedString::from(detail))
                    .size(Size::Xs)
                    .dimmed(),
            );
        if !hints.is_empty() {
            let mut hint_list = div().flex().flex_col().gap_1().pt(px(4.0));
            for (index, (agent, command)) in hints.into_iter().enumerate() {
                hint_list = hint_list.child(hint_row(index, agent, command));
            }
            left = left.child(hint_list);
        }
        rows = rows.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_3()
                .py(px(3.0))
                .child(left)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .flex_none()
                        .children(
                            action.map(|action| action_button(index, action, actions.clone())),
                        )
                        .child(
                            Badge::new(status_label)
                                .color(color)
                                .variant(Variant::Light),
                        ),
                ),
        );
    }
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .border_1()
        .rounded(px(6.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(Title::new("Setup doctor").order(4))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_1()
                        .child(
                            Button::new("setup-settings", "Agent settings")
                                .size(Size::Xs)
                                .variant(Variant::Subtle)
                                .on_click(move |_, window, cx| {
                                    settings.update(cx, |root, cx| {
                                        root.open_view(crate::state::View::Settings, window, cx);
                                        cx.notify();
                                    });
                                }),
                        )
                        .child(
                            Button::new("hide-setup", "Hide")
                                .size(Size::Xs)
                                .variant(Variant::Subtle)
                                .on_click(move |_, _, cx| {
                                    hide.update(cx, |root, cx| {
                                        root.setup_open = false;
                                        cx.notify();
                                    });
                                }),
                        ),
                ),
        )
        .child(rows)
        .into_any_element()
}

/// One copy-pasteable install hint: the agent name and its install command,
/// with a one-click button that copies just the command to the clipboard.
fn hint_row(index: usize, agent: String, command: String) -> impl IntoElement {
    let clip = command.clone();
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_col()
                .min_w(px(0.0))
                .child(Text::new(SharedString::from(agent)).size(Size::Xs).dimmed())
                .child(Text::new(SharedString::from(command)).size(Size::Xs)),
        )
        .child(
            Button::new(SharedString::from(format!("copy-hint-{index}")), "Copy")
                .size(Size::Xs)
                .variant(Variant::Subtle)
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(clip.clone()));
                }),
        )
}

/// The decision button beside a check.
///
/// Trusting routes through the confirm bar so the exact commands are restated
/// before permission is granted; revoking applies immediately, because
/// withdrawing a permission does not need a second confirmation.
fn action_button(index: usize, action: CheckAction, handle: Entity<Root>) -> impl IntoElement {
    let label = match action {
        CheckAction::TrustProject(_) => "Trust",
        CheckAction::RevokeProjectTrust(_) => "Revoke trust",
    };
    Button::new(SharedString::from(format!("setup-action-{index}")), label)
        .size(Size::Xs)
        .variant(match action {
            CheckAction::TrustProject(_) => Variant::Light,
            CheckAction::RevokeProjectTrust(_) => Variant::Subtle,
        })
        .on_click(move |_, _, cx| {
            handle.update(cx, |root, _cx| match action {
                CheckAction::TrustProject(id) => root.request_trust_project(id),
                CheckAction::RevokeProjectTrust(id) => root.revoke_project_trust(id),
            });
        })
}
