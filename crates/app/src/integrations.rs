//! The Integrations surface: GitHub pull requests and issues (via `gh`), with a
//! Refresh action. An issue's row previews the branch a "open worktree" flow
//! would create. Linear appears as a configured-or-not status line.

use gpui::prelude::*;
use gpui::{div, px, App, Entity, IntoElement, SharedString, Window};
use guise::prelude::*;

use crate::control::Button;
use crate::state::Root;
use github::{Issue, PullRequest};

#[allow(clippy::too_many_arguments)]
pub fn integrations_view(
    prs: Vec<PullRequest>,
    issues: Vec<Issue>,
    linear_issues: Vec<linear::Issue>,
    linear_configured: bool,
    error: Option<String>,
    handle: Entity<Root>,
    _window: &mut Window,
    _cx: &mut App,
) -> impl IntoElement {
    let mut col = div().flex().flex_col().w_full().gap_4().p(px(20.0));

    let refresh = handle.clone();
    col = col.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(Title::new("Integrations").order(2))
            .child(
                Button::new("gh-refresh", "Refresh from GitHub")
                    .size(Size::Xs)
                    .variant(Variant::Filled)
                    .on_click(move |_, _, cx| {
                        refresh.update(cx, |root, cx| {
                            root.load_github();
                            cx.notify();
                        });
                    }),
            ),
    );

    if let Some(err) = error {
        col = col.child(
            Alert::new(SharedString::from(format!(
                "Check that the GitHub CLI is installed and signed in, then refresh. {err}"
            )))
            .title("Could not load GitHub")
            .color(ColorName::Yellow),
        );
    }

    // Pull requests.
    col = col.child(Title::new("Pull requests").order(4));
    if prs.is_empty() {
        col = col.child(
            Text::new("Pull requests created for this repository appear here so you can follow work through review. None are open right now.")
                .size(Size::Sm)
                .dimmed(),
        );
    } else {
        let mut list = div().flex().flex_col().gap_2();
        for pr in prs {
            list = list.child(pr_row(pr));
        }
        col = col.child(list);
    }

    // Issues.
    col = col.child(Title::new("Issues").order(4));
    if issues.is_empty() {
        col = col.child(
            Text::new("GitHub issues appear here so you can turn reported work into an isolated worktree. None are open right now.")
                .size(Size::Sm)
                .dimmed(),
        );
    } else {
        let mut list = div().flex().flex_col().gap_2();
        for issue in issues {
            list = list.child(issue_row(issue, handle.clone()));
        }
        col = col.child(list);
    }

    // Linear.
    col = col.child(Title::new("Linear").order(4));
    if !linear_configured {
        let settings = handle.clone();
        col = col.child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    Text::new("Connect Linear to browse issues and start work from them.")
                        .size(Size::Sm)
                        .dimmed(),
                )
                .child(
                    Button::new("linear-settings", "Open settings")
                        .size(Size::Xs)
                        .variant(Variant::Subtle)
                        .on_click(move |_, window, cx| {
                            settings.update(cx, |root, cx| {
                                root.open_view(crate::state::View::Settings, window, cx);
                                cx.notify();
                            });
                        }),
                ),
        );
    } else if linear_issues.is_empty() {
        col = col.child(
            Text::new("Linear issues appear here so you can start a task directly from planned work. Refresh to load them.")
                .size(Size::Sm)
                .dimmed(),
        );
    } else {
        let mut list = div().flex().flex_col().gap_2();
        for issue in linear_issues {
            list = list.child(linear_row(issue, handle.clone()));
        }
        col = col.child(list);
    }

    col
}

fn linear_row(issue: linear::Issue, handle: Entity<Root>) -> impl IntoElement {
    let for_click = issue.clone();
    Card::new().padding(Size::Sm).child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(Text::new(SharedString::from(issue.identifier.clone())).dimmed())
            .child(Text::new(SharedString::from(issue.title.clone())).bold())
            .child(
                Text::new(SharedString::from(issue.state.clone()))
                    .size(Size::Xs)
                    .dimmed(),
            )
            .child(
                Button::new(
                    SharedString::from(format!("linear-wt-{}", issue.identifier)),
                    "Open worktree",
                )
                .size(Size::Xs)
                .variant(Variant::Light)
                .on_click(move |_, _, cx| {
                    let issue = for_click.clone();
                    handle.update(cx, |root, cx| {
                        root.create_worktree_from_linear_issue(&issue);
                        cx.notify();
                    });
                }),
            ),
    )
}

fn pr_row(pr: PullRequest) -> impl IntoElement {
    let badge = if pr.draft {
        Badge::new("draft")
            .color(ColorName::Gray)
            .variant(Variant::Light)
    } else {
        Badge::new("open")
            .color(ColorName::Green)
            .variant(Variant::Light)
    };
    Card::new().padding(Size::Sm).child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(Text::new(SharedString::from(format!("#{}", pr.number))).dimmed())
            .child(Text::new(SharedString::from(pr.title.clone())).bold())
            .child(badge)
            .child(
                Text::new(SharedString::from(format!("{} → {}", pr.head, pr.base)))
                    .size(Size::Xs)
                    .dimmed(),
            ),
    )
}

fn issue_row(issue: Issue, handle: Entity<Root>) -> impl IntoElement {
    let branch = github::issue_branch(&issue);
    let for_click = issue.clone();
    Card::new().padding(Size::Sm).child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(Text::new(SharedString::from(format!("#{}", issue.number))).dimmed())
            .child(Text::new(SharedString::from(issue.title.clone())).bold())
            .child(
                Text::new(SharedString::from(format!("→ {branch}")))
                    .size(Size::Xs)
                    .dimmed(),
            )
            .child(
                Button::new(
                    SharedString::from(format!("issue-wt-{}", issue.number)),
                    "Open worktree",
                )
                .size(Size::Xs)
                .variant(Variant::Light)
                .on_click(move |_, _, cx| {
                    let issue = for_click.clone();
                    handle.update(cx, |root, cx| {
                        root.create_worktree_from_issue(&issue);
                        cx.notify();
                    });
                }),
            ),
    )
}
