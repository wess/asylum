//! The Integrations surface: GitHub pull requests and issues (via `gh`), with a
//! Refresh action. An issue's row previews the branch a "open worktree" flow
//! would create. Linear appears as a configured-or-not status line.

use gpui::prelude::*;
use gpui::{div, px, App, Entity, IntoElement, SharedString, Window};
use guise::prelude::*;

use crate::control::Button;
use crate::state::Root;
use github::{Issue, PullRequest};

pub fn integrations_view(
    prs: Vec<PullRequest>,
    issues: Vec<Issue>,
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
            Alert::new(SharedString::from(format!("GitHub: {err}"))).color(ColorName::Yellow),
        );
    }

    // Pull requests.
    col = col.child(Title::new("Pull requests").order(4));
    if prs.is_empty() {
        col = col.child(
            Text::new("No open pull requests loaded.")
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
        col = col.child(Text::new("No open issues loaded.").size(Size::Sm).dimmed());
    } else {
        let mut list = div().flex().flex_col().gap_2();
        for issue in issues {
            list = list.child(issue_row(issue, handle.clone()));
        }
        col = col.child(list);
    }

    // Linear.
    col = col.child(Title::new("Linear").order(4));
    col = col.child(
        Text::new("Set a Linear API token in settings to browse teams, projects, and issues.")
            .size(Size::Sm)
            .dimmed(),
    );

    col
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
