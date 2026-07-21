//! The Inbox surface: notifications newest-first, unread ones marked. A button
//! marks everything read.

use gpui::prelude::*;
use gpui::{div, px, App, Entity, IntoElement, SharedString, Window};
use guise::prelude::*;

use crate::control::{empty, Button};
use crate::state::Root;
use store::Notification;

pub fn inbox_view(
    items: Vec<Notification>,
    handle: Entity<Root>,
    _window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let mut col = div()
        .id("inbox-scroll")
        .flex()
        .flex_col()
        .size_full()
        .gap_4()
        .p(px(20.0))
        .overflow_y_scroll();

    let clear = handle.clone();
    col = col.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(Title::new("Inbox").order(2))
            .child(
                Button::new("mark-all", "Mark all as read")
                    .size(Size::Xs)
                    .variant(Variant::Subtle)
                    .disabled(items.iter().all(|item| item.read))
                    .on_click(move |_, _, cx| {
                        clear.update(cx, |root, cx| {
                            if let Err(error) = root.db.mark_all_read() {
                                root.push_error("Could not update inbox", error.to_string());
                            }
                            cx.notify();
                        });
                    }),
            ),
    );

    if items.is_empty() {
        return col.child(empty(
            "You’re all caught up",
            "Run updates, completed checks, and anything that needs your attention will appear here.",
        ));
    }

    let focus = guise::theme::theme(cx).primary().hsla();
    for n in items {
        col = col.child(notification_row(n, handle.clone(), focus));
    }
    col
}

fn notification_row(n: Notification, handle: Entity<Root>, focus: gpui::Hsla) -> impl IntoElement {
    let id = n.id;
    let label = format!("Mark {} as read", n.title);
    let color = match n.kind.as_str() {
        "check_failed" | "run_failed" => ColorName::Red,
        "attention" => ColorName::Orange,
        _ => ColorName::Blue,
    };
    let dot = if n.read { "○" } else { "●" };
    let update = (n.kind.as_str() == "update").then(|| parse_update_body(&n.body));

    let summary = match &update {
        Some((preview, _)) => {
            let headline = truncate_chars(&preview.replace('\n', " "), HEADLINE_CHARS);
            format!("{headline}\n\n{UPDATE_DISCLAIMER}")
        }
        None => n.body.clone(),
    };

    let mut info = div()
        .flex()
        .flex_col()
        .child(Text::new(SharedString::from(n.title.clone())).bold())
        .child(
            Text::new(SharedString::from(summary))
                .size(Size::Xs)
                .dimmed(),
        );

    if let Some((preview, url)) = update {
        let has_url = url.is_some();
        info = info.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .pt(px(6.0))
                .child(
                    Button::new(
                        SharedString::from(format!("view-release-{id}")),
                        "View release",
                    )
                    .size(Size::Xs)
                    .variant(Variant::Light)
                    .disabled(!has_url)
                    .on_click(move |_, _, _cx| {
                        if let Some(url) = &url {
                            open_url(url);
                        }
                    }),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("release-notes-tip-{id}")))
                        .tooltip(guise::tooltip(preview))
                        .child(
                            Button::new(
                                SharedString::from(format!("release-notes-{id}")),
                                "Release notes",
                            )
                            .size(Size::Xs)
                            .variant(Variant::Subtle),
                        ),
                ),
        );
    }

    div()
        .id(SharedString::from(format!("notif-{id}")))
        .cursor_pointer()
        .tab_index(0)
        .role(gpui::accesskit::Role::Button)
        .aria_label(SharedString::from(label))
        .focus_visible(move |style| style.border_1().border_color(focus))
        .on_click(move |_, _, cx| {
            handle.update(cx, |root, cx| {
                if let Err(error) = root.db.mark_read(id, true) {
                    root.push_error("Could not update notification", error.to_string());
                }
                cx.notify();
            });
        })
        .child(
            Card::new().padding(Size::Sm).child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(Text::new(SharedString::from(dot)))
                    .child(
                        Badge::new(SharedString::from(n.kind.clone()))
                            .color(color)
                            .variant(Variant::Light),
                    )
                    .child(info),
            ),
        )
}

/// The no-self-update stance, appended to every update notification's body:
/// Asylum never swaps its own binary, so this says where to actually get one.
const UPDATE_DISCLAIMER: &str = "Update via brew/scoop or the downloads page.";

/// First lines kept from the release notes.
const PREVIEW_LINES: usize = 4;
/// Char cap on the kept preview (well under the update crate's 4 KiB cache).
const PREVIEW_CHARS: usize = 480;
/// Char cap on the always-visible row summary; the tooltip shows the rest of
/// the preview.
const HEADLINE_CHARS: usize = 120;

/// The stored body for an "update" notification: a plain-text notes preview,
/// the no-self-update line, and the release URL — parsed back out by the
/// Inbox row's actions via [`parse_update_body`].
pub(crate) fn update_body(notes: &str, url: &str) -> String {
    let preview = notes_preview(notes);
    if preview.is_empty() {
        format!("{UPDATE_DISCLAIMER} Release: {url}")
    } else {
        format!("{preview}\n\n{UPDATE_DISCLAIMER} Release: {url}")
    }
}

/// Pulls the notes preview and release URL back out of an "update"
/// notification's body (see [`update_body`]). Defensive: text that doesn't
/// match the expected shape is returned whole as the preview with no URL,
/// rather than panicking.
fn parse_update_body(body: &str) -> (String, Option<String>) {
    let preview = body
        .split(UPDATE_DISCLAIMER)
        .next()
        .unwrap_or(body)
        .trim()
        .to_string();
    let url = body
        .rsplit_once("Release: ")
        .map(|(_, rest)| rest.trim())
        .filter(|u| !u.is_empty())
        .map(str::to_string);
    (preview, url)
}

/// Plain-text-ifies `notes` (crudely strips Markdown headers and links) and
/// keeps only the first few lines, so a changelog fits a notification.
fn notes_preview(notes: &str) -> String {
    let lines: Vec<String> = notes
        .lines()
        .map(strip_markdown_line)
        .filter(|l| !l.is_empty())
        .take(PREVIEW_LINES)
        .collect();
    truncate_chars(&lines.join("\n"), PREVIEW_CHARS)
}

/// Crude, safe Markdown-to-plain-text for one line: drops a leading `#`
/// header marker and turns `[label](url)` links into just `label`. Anything
/// else is left as-is — this is a preview, not a renderer. Every slice point
/// below comes from `find`/`strip_prefix` on single-byte ASCII delimiters, so
/// it never lands mid-character.
fn strip_markdown_line(line: &str) -> String {
    let line = line.trim().trim_start_matches('#').trim();
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(start) = rest.find('[') {
        out.push_str(&rest[..start]);
        let after_bracket = &rest[start + 1..];
        let Some(close) = after_bracket.find(']') else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let link_label = &after_bracket[..close];
        let after_label = &after_bracket[close + 1..];
        match after_label
            .strip_prefix('(')
            .and_then(|s| s.find(')').map(|end| (s, end)))
        {
            Some((url_start, end)) => {
                out.push_str(link_label);
                rest = &url_start[end + 1..];
            }
            None => {
                out.push('[');
                out.push_str(link_label);
                out.push(']');
                rest = after_label;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Truncates `text` to at most `max_chars` characters, marking truncation
/// with an ellipsis. Char-counted, not byte-sliced, so it is UTF-8 safe.
fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut truncated: String = text.chars().take(max_chars).collect();
    truncated.push('…');
    truncated
}

/// Opens a URL in the system browser (same idiom as the app's other external
/// links).
fn open_url(url: &str) {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(program).arg(url).spawn();
}

#[cfg(test)]
#[path = "../tests/notifications.rs"]
mod tests;
