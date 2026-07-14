//! First-run and project readiness checks shown before task dispatch.

use std::path::Path;

use gpui::prelude::*;
use gpui::{div, px, Entity, IntoElement, SharedString};
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
}

impl Check {
    fn new(label: impl Into<String>, detail: impl Into<String>, status: Status) -> Self {
        Self {
            label: label.into(),
            detail: detail.into(),
            status,
        }
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
        Check::new(
            "Agents",
            "No configured agent executable was found on PATH.",
            Status::Blocked,
        )
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
    if !project_config.setup.is_empty() {
        checks.push(Check::new(
            "Worktree setup",
            format!(
                "{} setup command(s) will run before each agent starts",
                project_config.setup.len()
            ),
            Status::Attention,
        ));
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
                        format!("Setup: {blocked} blocked")
                    } else if attention > 0 {
                        format!("Setup: {attention} to verify")
                    } else {
                        "Setup ready".to_string()
                    }),
                )
                .size(Size::Xs)
                .variant(if blocked > 0 {
                    Variant::Filled
                } else {
                    Variant::Subtle
                })
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
    let hide = handle;
    let mut rows = div().flex().flex_col().gap_1();
    for check in checks {
        let (label, color) = match check.status {
            Status::Pass => ("ready", ColorName::Green),
            Status::Attention => ("verify", ColorName::Orange),
            Status::Blocked => ("blocked", ColorName::Red),
        };
        rows = rows.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_3()
                .py(px(3.0))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w(px(0.0))
                        .flex_1()
                        .child(Text::new(SharedString::from(check.label)).size(Size::Sm))
                        .child(
                            Text::new(SharedString::from(check.detail))
                                .size(Size::Xs)
                                .dimmed(),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .child(Badge::new(label).color(color).variant(Variant::Light)),
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
