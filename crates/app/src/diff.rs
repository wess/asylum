//! The diff review surface: render a run's changes as an annotatable unified
//! diff. Added lines get a green wash, removed lines a red one, with old/new
//! line numbers in the gutter. Click any line to anchor a comment there;
//! comments render inline under their line and ship back to the agent.

use std::collections::HashSet;
use std::path::PathBuf;

use gpui::prelude::*;
use gpui::{div, px, App, Entity, Hsla, IntoElement, SharedString, Window};
use guise::prelude::*;
use guise::surface;

use crate::control::{empty, Button};
use crate::icons::icon;
use crate::state::Root;
use crate::state::RunRow;
use checks::{CheckResult, Status};
use git::{DiffFile, DiffHunk, LineKind};
use store::{Annotation, Side};

/// The diff line a pending comment anchors to: (file, line, side).
pub type Target = Option<(String, u32, Side)>;

/// Per-hunk staging state for the review surface. A hunk is identified by its
/// file path and old-side start line — the key `git diff --cached` and the shown
/// diff agree on, since both measure the old side from `HEAD`.
pub struct StagingState {
    /// `(path, hunk.old_start)` of every hunk currently staged in the run's
    /// worktree index.
    pub staged: HashSet<(String, u32)>,
    /// True when the run's worktree has uncommitted changes to stage; false
    /// hides the affordances (e.g. an already-committed or merged run).
    pub active: bool,
}

impl StagingState {
    /// The empty, inactive state (no run, or an unreadable worktree).
    pub fn inactive() -> Self {
        Self {
            staged: HashSet::new(),
            active: false,
        }
    }

    fn is_staged(&self, path: &str, hunk: &DiffHunk) -> bool {
        self.staged.contains(&(path.to_string(), hunk.old_start))
    }
}

impl Root {
    /// The worktree of the run currently under review, when it is a live git
    /// worktree — the target of every stage/unstage action.
    fn review_worktree(&self) -> Option<PathBuf> {
        let rid = self.current_run_id()?;
        let run = self.db.run(rid).ok()?;
        let wt = PathBuf::from(run.worktree);
        git::is_repo(&wt).then_some(wt)
    }

    /// Compute the per-hunk staging state for the review surface from git (never
    /// from shadow state), so it always reflects the real index.
    pub fn review_staging(&self) -> StagingState {
        let Some(wt) = self.review_worktree() else {
            return StagingState::inactive();
        };
        let staged = git::stage::staged(&wt)
            .unwrap_or_default()
            .into_iter()
            .flat_map(|file| {
                file.hunks
                    .into_iter()
                    .map(move |hunk| (file.path.clone(), hunk.old_start))
            })
            .collect();
        let active = git::stage::has_worktree_changes(&wt).unwrap_or(false);
        StagingState { staged, active }
    }

    /// Stage the single hunk carried by `file` (its first and only hunk).
    pub fn stage_review_hunk(&mut self, file: &DiffFile) {
        let Some(wt) = self.review_worktree() else {
            return;
        };
        let Some(hunk) = file.hunks.first() else {
            return;
        };
        if let Err(error) = git::stage::stage_hunk(&wt, file, hunk) {
            self.push_error("Could not stage hunk", error.to_string());
        }
    }

    /// Unstage the single hunk carried by `file` (its first and only hunk).
    pub fn unstage_review_hunk(&mut self, file: &DiffFile) {
        let Some(wt) = self.review_worktree() else {
            return;
        };
        let Some(hunk) = file.hunks.first() else {
            return;
        };
        if let Err(error) = git::stage::unstage_hunk(&wt, file, hunk) {
            self.push_error("Could not unstage hunk", error.to_string());
        }
    }

    /// Stage every hunk of `file`.
    pub fn stage_review_file(&mut self, file: &DiffFile) {
        let Some(wt) = self.review_worktree() else {
            return;
        };
        if let Err(error) = git::stage::stage_file(&wt, file) {
            self.push_error("Could not stage file", error.to_string());
        }
    }

    /// Unstage every hunk of `file`.
    pub fn unstage_review_file(&mut self, file: &DiffFile) {
        let Some(wt) = self.review_worktree() else {
            return;
        };
        if let Err(error) = git::stage::unstage_file(&wt, file) {
            self.push_error("Could not unstage file", error.to_string());
        }
    }
}

/// Wash/accent colors pulled from the active guise theme, for the raw diff
/// backgrounds and focus rings guise's components don't cover directly (the
/// badges elsewhere in this file resolve their own colors through the same
/// `ColorName` + `Variant::Light` pairing).
struct Palette {
    /// Added-line background wash.
    added: Hsla,
    /// Removed-line background wash.
    removed: Hsla,
    /// Context-line (no change) background - fully transparent.
    context: Hsla,
    /// Opaque brand color, for focus rings and other solid accents.
    primary: Hsla,
    /// The line just clicked, awaiting a new comment.
    targeted: Hsla,
    /// An existing review comment's row background.
    comment: Hsla,
    /// Secondary/dimmed tint, for quiet inline glyphs.
    dimmed: Hsla,
    /// Solid green foreground, for the staged-hunk indicator.
    staged: Hsla,
}

fn palette(cx: &App) -> Palette {
    let t = guise::theme::theme(cx);
    let primary = t.primary().hsla();
    Palette {
        added: surface(t, ColorName::Green, Variant::Light).bg,
        removed: surface(t, ColorName::Red, Variant::Light).bg,
        context: gpui::transparent_black(),
        primary,
        targeted: surface(t, t.primary_color, Variant::Light).bg,
        comment: t.primary().alpha(0.10),
        dimmed: t.dimmed().hsla(),
        staged: surface(t, ColorName::Green, Variant::Light).fg,
    }
}

/// Build the diff review content from the parsed files.
#[allow(clippy::too_many_arguments)]
pub fn review(
    files: Vec<DiffFile>,
    check_results: Vec<CheckResult>,
    checking: bool,
    annotations: Vec<Annotation>,
    target: Target,
    branches: Vec<git::Branch>,
    runs: Vec<RunRow>,
    split: bool,
    staging: StagingState,
    note: Entity<guise::TextInput>,
    handle: Entity<Root>,
    _window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let p = palette(cx);
    let total_hunks: usize = files.iter().map(|file| file.hunks.len()).sum();
    let mut staged_hunks = 0usize;
    for file in &files {
        for hunk in &file.hunks {
            if staging.is_staged(&file.path, hunk) {
                staged_hunks += 1;
            }
        }
    }
    let mut col = div()
        .id("diff-scroll")
        .flex()
        .flex_col()
        .size_full()
        .gap_4()
        .p(px(20.0))
        .overflow_y_scroll();
    col = col.child(
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_2()
            .justify_between()
            .child({
                let mut title = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(Title::new("Review").order(2));
                if staging.active && staged_hunks > 0 {
                    let color = if staged_hunks == total_hunks {
                        ColorName::Green
                    } else {
                        ColorName::Blue
                    };
                    title = title.child(
                        Badge::new(SharedString::from(format!(
                            "{staged_hunks} of {total_hunks} hunks staged"
                        )))
                        .color(color)
                        .variant(Variant::Light),
                    );
                }
                title
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child({
                        let toggle = handle.clone();
                        Button::new(
                            "diff-view-toggle",
                            if split {
                                "Unified view"
                            } else {
                                "Side-by-side"
                            },
                        )
                        .size(Size::Xs)
                        .variant(Variant::Subtle)
                        .on_click(move |_, _, cx| {
                            toggle.update(cx, |root, cx| {
                                root.diff_split = !root.diff_split;
                                cx.notify();
                            });
                        })
                    })
                    .child(checks_bar(check_results, checking, handle.clone())),
            ),
    );

    let has_runs = !runs.is_empty();
    if !branches.is_empty() {
        let mut chips = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_1()
            .child(Text::new("Branches:").size(Size::Xs).dimmed());
        for b in branches.into_iter().take(12) {
            let color = if b.head {
                ColorName::Green
            } else {
                ColorName::Gray
            };
            chips = chips.child(
                Badge::new(SharedString::from(b.name))
                    .color(color)
                    .variant(Variant::Light),
            );
        }
        col = col.child(chips);
    }

    if has_runs {
        let mut row = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_1()
            .child(Text::new("Compare:").size(Size::Xs).dimmed());
        for run in runs {
            let select = handle.clone();
            let id = run.id;
            let name = agent::find(&run.agent)
                .map(|agent| agent.name)
                .unwrap_or(run.agent.as_str());
            row = row.child(
                Button::new(
                    SharedString::from(format!("diff-run-{id}")),
                    SharedString::from(format!("{}{}", if run.selected { "* " } else { "" }, name)),
                )
                .size(Size::Xs)
                .variant(if run.selected {
                    Variant::Filled
                } else {
                    Variant::Light
                })
                .on_click(move |_, _, cx| {
                    select.update(cx, |root, cx| {
                        root.select_run(id);
                        cx.notify();
                    });
                }),
            );
        }
        col = col.child(row);
    }

    if has_runs {
        col = col.child(comment_panel(
            &annotations,
            target.clone(),
            note,
            handle.clone(),
        ));
    }

    if files.is_empty() {
        let (title, detail) = if !has_runs {
            (
                "Choose a run to review",
                "Start or select an agent run, then return here to inspect its changes and leave line comments.",
            )
        } else {
            (
                "No file changes yet",
                "This run has not changed tracked files. Check its terminal output for progress or an explanation.",
            )
        };
        return col.child(empty(title, detail));
    }

    for file in files {
        col = if split {
            col.child(file_block_split(file, &staging, handle.clone(), &p))
        } else {
            col.child(file_block(
                file,
                &annotations,
                &target,
                &staging,
                handle.clone(),
                &p,
            ))
        };
    }
    col
}

/// The review-comment panel: where the next comment lands, the input to add
/// it, and the button that ships the open batch back to an agent.
fn comment_panel(
    annotations: &[Annotation],
    target: Target,
    note: Entity<guise::TextInput>,
    handle: Entity<Root>,
) -> impl IntoElement {
    let mut panel = div().flex().flex_col().gap_2();

    let open = annotations.iter().filter(|a| !a.resolved).count();
    let anchor = match &target {
        Some((file, line, _)) => format!("Commenting on {file}:{line}"),
        None => "Click a diff line to anchor your comment.".to_string(),
    };
    let mut status = div().flex().flex_row().items_center().gap_2().child(
        Text::new(SharedString::from(anchor))
            .size(Size::Xs)
            .dimmed(),
    );
    if open > 0 {
        let send = handle.clone();
        status = status
            .child(
                Badge::new(SharedString::from(format!("{open} open")))
                    .color(ColorName::Blue)
                    .variant(Variant::Light),
            )
            .child(
                Button::new("send-review", "Send review to agent")
                    .size(Size::Xs)
                    .variant(Variant::Filled)
                    .on_click(move |_, window, cx| {
                        send.update(cx, |root, cx| {
                            root.send_review_to_agent(window, cx);
                            cx.notify();
                        });
                    }),
            );
    }
    panel = panel.child(status);

    let add = handle;
    let note_read = note.clone();
    panel = panel.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(div().flex_1().child(note))
            .child(
                Button::new("add-note", "Add comment")
                    .size(Size::Xs)
                    .variant(Variant::Light)
                    .on_click(move |_, _, cx| {
                        let text = note_read.read(cx).text();
                        add.update(cx, |root, cx| {
                            root.add_review_note(&text);
                            cx.notify();
                        });
                        note_read.update(cx, |n, cx| n.set_text("", cx));
                    }),
            ),
    );

    Card::new().padding(Size::Sm).child(panel)
}

/// The checks bar: a Run button plus a PASS/FAIL badge per check.
fn checks_bar(results: Vec<CheckResult>, checking: bool, handle: Entity<Root>) -> impl IntoElement {
    let mut row = div().flex().flex_row().items_center().gap_2();
    for r in &results {
        let (color, label) = match r.status {
            Status::Pass => (ColorName::Green, "PASS"),
            Status::Fail => (ColorName::Red, "FAIL"),
            Status::Skipped => (ColorName::Gray, "skip"),
        };
        row = row.child(
            Badge::new(SharedString::from(format!("{} {label}", r.id)))
                .color(color)
                .variant(Variant::Light),
        );
    }
    row = row.child(
        Button::new(
            "run-checks",
            if checking {
                "Checks running"
            } else {
                "Run checks"
            },
        )
        .size(Size::Xs)
        .variant(Variant::Filled)
        .disabled(checking)
        .on_click(move |_, _, cx| {
            handle.update(cx, |root, cx| {
                root.run_checks(cx);
                cx.notify();
            });
        }),
    );
    row
}

fn file_block(
    file: DiffFile,
    annotations: &[Annotation],
    target: &Target,
    staging: &StagingState,
    handle: Entity<Root>,
    p: &Palette,
) -> impl IntoElement {
    let header = file_header(&file, staging, handle.clone());

    let mut body = div()
        .flex()
        .flex_col()
        .w_full()
        .font_family("monospace")
        .text_size(px(12.0));

    for (hi, hunk) in file.hunks.iter().enumerate() {
        body = body.child(hunk_header(&file, hunk, staging, handle.clone(), p));
        for (li, line) in hunk.lines.iter().enumerate() {
            let (line_no, side) = anchor_of(line);
            let targeted = matches!(
                (target, line_no),
                (Some((f, l, s)), Some(n)) if *f == file.path && *l == n && *s == side
            );
            body = body.child(diff_line(
                line,
                SharedString::from(format!("dl-{}-{hi}-{li}", file.path)),
                file.path.clone(),
                line_no,
                side,
                targeted,
                handle.clone(),
                p,
            ));
            if let Some(n) = line_no {
                for a in annotations
                    .iter()
                    .filter(|a| a.file == file.path && a.line == n && a.side == side)
                {
                    body = body.child(annotation_row(a, handle.clone(), p));
                }
            }
        }
    }

    Card::new().padding(Size::Md).child(
        div()
            .flex()
            .flex_col()
            .w_full()
            .gap_2()
            .child(header)
            .child(Divider::new())
            .child(body),
    )
}

/// A read-only side-by-side rendering of one file: removed lines on the left,
/// added lines on the right, context spanning both. Annotating stays in the
/// unified view, which owns the click-to-comment interaction.
fn file_block_split(
    file: DiffFile,
    staging: &StagingState,
    handle: Entity<Root>,
    p: &Palette,
) -> impl IntoElement {
    let header = file_header(&file, staging, handle.clone());

    let mut body = div()
        .flex()
        .flex_col()
        .w_full()
        .font_family("monospace")
        .text_size(px(12.0));

    for hunk in &file.hunks {
        body = body.child(hunk_header(&file, hunk, staging, handle.clone(), p));
        // Pair each run of removed lines with the following added lines so a
        // change shows old and new side by side; context flushes the pairing.
        let mut removed_run: Vec<&git::DiffLine> = Vec::new();
        let mut added_run: Vec<&git::DiffLine> = Vec::new();
        let flush = |body: gpui::Div,
                     removed_run: &mut Vec<&git::DiffLine>,
                     added_run: &mut Vec<&git::DiffLine>| {
            let rows = removed_run.len().max(added_run.len());
            let mut body = body;
            for i in 0..rows {
                body = body.child(split_row(
                    removed_run.get(i).copied(),
                    added_run.get(i).copied(),
                    p,
                ));
            }
            removed_run.clear();
            added_run.clear();
            body
        };
        for line in &hunk.lines {
            match line.kind {
                LineKind::Removed => removed_run.push(line),
                LineKind::Added => added_run.push(line),
                LineKind::Context => {
                    body = flush(body, &mut removed_run, &mut added_run);
                    body = body.child(split_row(Some(line), Some(line), p));
                }
            }
        }
        body = flush(body, &mut removed_run, &mut added_run);
    }

    Card::new().padding(Size::Md).child(
        div()
            .flex()
            .flex_col()
            .w_full()
            .gap_2()
            .child(header)
            .child(Divider::new())
            .child(body),
    )
}

/// One row of the side-by-side view: an old cell and a new cell.
fn split_row(
    left: Option<&git::DiffLine>,
    right: Option<&git::DiffLine>,
    p: &Palette,
) -> impl IntoElement {
    let cell = |line: Option<&git::DiffLine>, removed: bool| {
        let (bg, no, content) = match line {
            Some(l) if removed && l.kind == LineKind::Removed => {
                (p.removed, l.old_no, l.content.clone())
            }
            Some(l) if !removed && l.kind == LineKind::Added => {
                (p.added, l.new_no, l.content.clone())
            }
            Some(l) if l.kind == LineKind::Context => (
                p.context,
                if removed { l.old_no } else { l.new_no },
                l.content.clone(),
            ),
            _ => (p.context, None, String::new()),
        };
        let gutter = match no {
            Some(v) => format!("{v:>4}"),
            None => "    ".to_string(),
        };
        div()
            .flex()
            .flex_row()
            .w_1_2()
            .bg(bg)
            .child(
                div()
                    .px(px(6.0))
                    .child(Text::new(SharedString::from(gutter)).dimmed()),
            )
            .child(Text::new(SharedString::from(content)))
    };
    div()
        .flex()
        .flex_row()
        .w_full()
        .gap_2()
        .child(cell(left, true))
        .child(cell(right, false))
}

/// The file header row: path + added/removed badges on the left, and - when
/// staging is active - a file-level stage/unstage control on the right.
fn file_header(file: &DiffFile, staging: &StagingState, handle: Entity<Root>) -> impl IntoElement {
    let (added, removed) = file.line_stats();
    let left = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(Text::new(SharedString::from(file.path.clone())).bold())
        .child(
            Badge::new(format!("+{added}"))
                .color(ColorName::Green)
                .variant(Variant::Light),
        )
        .child(
            Badge::new(format!("-{removed}"))
                .color(ColorName::Red)
                .variant(Variant::Light),
        );
    let mut header = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .w_full()
        .justify_between()
        .child(left);
    if staging.active {
        header = header.child(file_stage_control(file, staging, handle));
    }
    header
}

/// The file-level stage control: one button that stages or unstages every hunk
/// of the file, plus a badge summarizing how many of its hunks are staged.
fn file_stage_control(
    file: &DiffFile,
    staging: &StagingState,
    handle: Entity<Root>,
) -> impl IntoElement {
    let total = file.hunks.len();
    let staged = file
        .hunks
        .iter()
        .filter(|hunk| staging.is_staged(&file.path, hunk))
        .count();
    let all = total > 0 && staged == total;
    let meta = file_meta(file);
    let mut row = div().flex().flex_row().items_center().gap_2().child(
        Button::new(
            SharedString::from(format!("stage-file-{}", file.path)),
            if all { "Unstage file" } else { "Stage file" },
        )
        .size(Size::Xs)
        .variant(Variant::Subtle)
        .on_click(move |_, _, cx| {
            handle.update(cx, |root, cx| {
                if all {
                    root.unstage_review_file(&meta);
                } else {
                    root.stage_review_file(&meta);
                }
                cx.notify();
            });
        }),
    );
    if staged > 0 {
        let (color, label) = if all {
            (ColorName::Green, "staged".to_string())
        } else {
            (ColorName::Blue, format!("{staged}/{total} staged"))
        };
        row = row.child(
            Badge::new(SharedString::from(label))
                .color(color)
                .variant(Variant::Light),
        );
    }
    row
}

/// One hunk's header line (`@@ -a +b @@ ...`), preceded by a stage/unstage
/// toggle when staging is active, or a staged indicator otherwise.
fn hunk_header(
    file: &DiffFile,
    hunk: &DiffHunk,
    staging: &StagingState,
    handle: Entity<Root>,
    p: &Palette,
) -> impl IntoElement {
    let is_staged = staging.is_staged(&file.path, hunk);
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .px(px(8.0))
        .py(px(2.0))
        .text_size(px(11.0));
    if staging.active {
        row = row.child(stage_toggle(file, hunk, is_staged, handle, p));
    } else if is_staged {
        row = row.child(icon("circle-check", 12.0).text_color(p.staged));
    }
    row.child(
        Text::new(SharedString::from(format!(
            "@@ -{} +{} @@ {}",
            hunk.old_start, hunk.new_start, hunk.header
        )))
        .dimmed(),
    )
}

/// The per-hunk stage/unstage affordance: a keyboard-operable circle that fills
/// green when the hunk is staged.
fn stage_toggle(
    file: &DiffFile,
    hunk: &DiffHunk,
    is_staged: bool,
    handle: Entity<Root>,
    p: &Palette,
) -> impl IntoElement {
    let single = single_file(file, hunk);
    let id = SharedString::from(format!("stage-{}-{}", file.path, hunk.old_start));
    let (glyph, label, color) = if is_staged {
        ("circle-check", "Unstage hunk", p.staged)
    } else {
        ("circle", "Stage hunk", p.dimmed)
    };
    let primary = p.primary;
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .tab_index(0)
        .role(gpui::accesskit::Role::Button)
        .aria_label(label)
        .tooltip(guise::tooltip(label))
        .focus_visible(move |style| style.border_1().border_color(primary))
        .child(icon(glyph, 13.0).text_color(color))
        .on_click(move |_, _, cx| {
            handle.update(cx, |root, cx| {
                if is_staged {
                    root.unstage_review_hunk(&single);
                } else {
                    root.stage_review_hunk(&single);
                }
                cx.notify();
            });
        })
}

/// A `DiffFile` carrying exactly one hunk - the payload the stage/unstage-hunk
/// actions operate on.
fn single_file(file: &DiffFile, hunk: &DiffHunk) -> DiffFile {
    DiffFile {
        path: file.path.clone(),
        old_path: file.old_path.clone(),
        status: file.status,
        hunks: vec![hunk.clone()],
        binary: file.binary,
    }
}

/// A `DiffFile` with just its identity - path, rename source, status - for the
/// file-level stage/unstage actions, which don't inspect hunks.
fn file_meta(file: &DiffFile) -> DiffFile {
    DiffFile {
        path: file.path.clone(),
        old_path: file.old_path.clone(),
        status: file.status,
        hunks: Vec::new(),
        binary: file.binary,
    }
}

/// Which (line number, side) a comment on this line anchors to.
fn anchor_of(line: &git::DiffLine) -> (Option<u32>, Side) {
    match line.kind {
        LineKind::Removed => (line.old_no, Side::Old),
        _ => (line.new_no, Side::New),
    }
}

#[allow(clippy::too_many_arguments)]
fn diff_line(
    line: &git::DiffLine,
    id: SharedString,
    file: String,
    line_no: Option<u32>,
    side: Side,
    targeted: bool,
    handle: Entity<Root>,
    p: &Palette,
) -> impl IntoElement {
    let (bg, sign) = match line.kind {
        LineKind::Added => (p.added, "+"),
        LineKind::Removed => (p.removed, "-"),
        LineKind::Context => (p.context, " "),
    };
    let gutter = |n: Option<u32>| match n {
        Some(v) => format!("{v:>4}"),
        None => "    ".to_string(),
    };
    let primary = p.primary;
    let mut row = div()
        .id(id)
        .flex()
        .flex_row()
        .w_full()
        .bg(if targeted { p.targeted } else { bg })
        .cursor_pointer()
        .child(
            div().px(px(6.0)).child(
                Text::new(SharedString::from(format!(
                    "{} {} ",
                    gutter(line.old_no),
                    gutter(line.new_no)
                )))
                .dimmed(),
            ),
        )
        .child(Text::new(SharedString::from(format!(
            "{sign} {}",
            line.content
        ))));
    if let Some(n) = line_no {
        row = row
            .tab_index(0)
            .role(gpui::accesskit::Role::Button)
            .aria_label(SharedString::from(format!("Comment on {file} line {n}")))
            .focus_visible(move |style| style.border_1().border_color(primary))
            .on_click(move |_, _, cx| {
                handle.update(cx, |root, cx| {
                    root.target_review_line(&file, n, side);
                    cx.notify();
                });
            });
    }
    row
}

/// An inline review comment under its diff line, with resolve/delete.
fn annotation_row(a: &Annotation, handle: Entity<Root>, p: &Palette) -> impl IntoElement {
    let (id, resolved) = (a.id, a.resolved);
    let text = if resolved {
        Text::new(SharedString::from(a.body.clone()))
            .size(Size::Xs)
            .dimmed()
    } else {
        Text::new(SharedString::from(a.body.clone())).size(Size::Xs)
    };
    let body = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .child(icon("message-square", 12.0).text_color(p.dimmed))
        .child(text);
    let primary = p.primary;
    let resolve = handle.clone();
    let del = handle;
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .pl(px(56.0))
        .pr(px(8.0))
        .py(px(3.0))
        .bg(p.comment)
        .child(div().flex_1().overflow_hidden().child(body))
        .child(
            div()
                .id(SharedString::from(format!("ann-res-{id}")))
                .px(px(4.0))
                .cursor_pointer()
                .tab_index(0)
                .role(gpui::accesskit::Role::Button)
                .aria_label(if resolved {
                    "Reopen comment"
                } else {
                    "Resolve comment"
                })
                .focus_visible(move |style| style.border_1().border_color(primary))
                .child(
                    Text::new(if resolved { "↺" } else { "✓" })
                        .size(Size::Xs)
                        .dimmed(),
                )
                .on_click(move |_, _, cx| {
                    resolve.update(cx, |root, cx| {
                        root.resolve_review_note(id, !resolved);
                        cx.notify();
                    });
                }),
        )
        .child(
            div()
                .id(SharedString::from(format!("ann-del-{id}")))
                .px(px(4.0))
                .cursor_pointer()
                .tab_index(0)
                .role(gpui::accesskit::Role::Button)
                .aria_label("Delete comment")
                .focus_visible(move |style| style.border_1().border_color(primary))
                .child(Text::new("×").size(Size::Xs).dimmed())
                .on_click(move |_, _, cx| {
                    del.update(cx, |root, cx| {
                        root.delete_review_note(id);
                        cx.notify();
                    });
                }),
        )
}
