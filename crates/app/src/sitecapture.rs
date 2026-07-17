//! Direct GPUI capture used to keep the website's product image current.

use std::{path::PathBuf, sync::Arc};

use gpui::{px, size, AppContext as _, VisualTestAppContext};

use crate::{icons, state::Root, theme, workspace::TabKind};

pub fn run(settings: &config::Settings, path: PathBuf) -> anyhow::Result<()> {
    let mut cx = VisualTestAppContext::with_asset_source(
        gpui_platform::current_platform(false),
        Arc::new(icons::Assets),
    );
    cx.update(|cx| theme::install(settings, cx));

    let window = cx.open_offscreen_window(size(px(1200.0), px(820.0)), |_window, cx| {
        cx.new(|_cx| {
            let mut root = Root::seeded();
            loadnotes(&mut root).expect("load sample notes");
            if std::env::var_os("ASYLUM_SITE_COLLAPSED").is_some() {
                root.note.files_open = false;
                root.note.details_open = false;
            }
            root.setup_open = false;
            if let Some(kind) = surface() {
                root.open_kind(kind);
            }
            root
        })
    })?;

    for _ in 0..2 {
        cx.run_until_parked();
        cx.update_window(window.into(), |_, window, _cx| window.refresh())?;
    }
    cx.run_until_parked();

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    cx.capture_screenshot(window.into())?.save(path)?;
    Ok(())
}

fn surface() -> Option<TabKind> {
    match std::env::var("ASYLUM_SITE_SURFACE").as_deref() {
        Ok("tasks") => None,
        Ok("diff") => Some(TabKind::Diff),
        Ok("integrations") => Some(TabKind::Integrations),
        Ok("accounts") => Some(TabKind::Accounts),
        Ok("settings") => Some(TabKind::Settings),
        _ => Some(TabKind::Notes),
    }
}

fn loadnotes(root: &mut Root) -> anyhow::Result<()> {
    let vault = std::env::temp_dir().join(format!("asylumsite{}", std::process::id()));
    notes::write(
        &vault,
        "asylum.md",
        "---\ntitle: Asylum product direction\ntype: project\nstatus: active\ntags:\n  - asylum\n  - product\n---\n\n# Asylum product direction\n\nAsylum is an agent development environment for running isolated attempts in parallel, reviewing the evidence, and merging the best result.\n\n## Current focus\n\n- Make fleet workflows obvious for first-time users.\n- Keep expert controls one command away.\n- Connect durable notes to tasks and runs.\n\n## Related\n\n- [[Architecture]]\n- [[Beginner workflow]]\n- [[SQLite decision]]\n",
    )?;
    notes::write(
        &vault,
        "architecture.md",
        "---\ntitle: Architecture\ntype: reference\ntags:\n  - engineering\n---\n\n# Architecture\n\nThe desktop shell coordinates worktrees, agents, reviews, and project knowledge.\n\nSee [[Asylum product direction]].\n",
    )?;
    notes::write(
        &vault,
        "guides/beginner.md",
        "---\ntitle: Beginner workflow\ntype: guide\ntags:\n  - tutorial\n---\n\n# Beginner workflow\n\nStart with one task and two agents, then compare checks and diffs.\n",
    )?;
    notes::write(
        &vault,
        "decisions/sqlite.md",
        "---\ntitle: SQLite decision\ntype: decision\nstatus: accepted\ntags:\n  - architecture\n---\n\n# SQLite decision\n\nUse SQLite for local durable state and simple backup.\n\nRelated to [[Asylum product direction]].\n",
    )?;
    root.note.project_id = root.project_id;
    root.note.root = vault.clone();
    root.note.index = notes::index(&vault)?;
    root.note.path = Some("asylum.md".to_string());
    Ok(())
}
