//! Direct GPUI capture used to keep the website's product image current.

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use anyhow::Context as _;
use gpui::{px, size, AppContext as _, VisualTestAppContext};

use crate::{icons, state::Root, theme, workspace::TabKind};

pub fn run(settings: &config::Settings, path: PathBuf) -> anyhow::Result<()> {
    let mut cx = VisualTestAppContext::with_asset_source(
        gpui_platform::current_platform(false),
        Arc::new(icons::Assets),
    );
    cx.update(|cx| theme::install(settings, cx));

    let width = dimension("ASYLUM_SITE_WIDTH", 1200.0);
    let height = dimension("ASYLUM_SITE_HEIGHT", 820.0);
    let window = cx.open_offscreen_window(size(px(width), px(height)), |_window, cx| {
        cx.new(|_cx| {
            let mut root = Root::seeded();
            if std::env::var_os("ASYLUM_SITE_DEMO").is_some() {
                loaddemo(&mut root).expect("load screenshot demo");
            }
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

fn dimension(name: &str, fallback: f32) -> f32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value: &f32| value.is_finite() && *value >= 640.0)
        .unwrap_or(fallback)
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

struct DemoRepo {
    root: PathBuf,
    runs: Vec<PathBuf>,
}

fn loaddemo(root: &mut Root) -> anyhow::Result<()> {
    if !root.db.projects()?.is_empty() {
        return Ok(());
    }
    let demo = demorepo()?;
    let now = crate::state::now();
    let project = root.db.create_project(
        "Lighthouse",
        &demo.root.to_string_lossy(),
        "main",
        now - 900,
    )?;
    root.db.set_project_trust(project.id, true, now - 900)?;
    root.db.touch_project(project.id, now)?;

    let task = root.db.create_task(
    project.id,
    "Make validation errors clear and actionable",
    "Replace the generic validation failure with field-specific messages. Add tests for empty names and malformed email addresses. Keep the public API stable.",
    now - 600,
  )?;
    root.db
        .set_task_status(task.id, store::TaskStatus::Running, now - 570)?;

    let reviewer = root.db.save_named_agent(
        project.id,
        "Reviewer",
        "Protect the public API and look for missing edge cases.",
        "codex",
        now - 800,
    )?;
    root.db
        .remember(reviewer.id, "Validation errors use stable field names.")?;
    root.db
        .remember(reviewer.id, "Every user-facing error needs a focused test.")?;

    let winner = root.db.create_run(
        task.id,
        "codex",
        &demo.runs[0].to_string_lossy(),
        "asylum/validation-codex",
    )?;
    root.db.start_run(winner.id, now - 420)?;
    root.db.finish_run_with_output(
    winner.id,
    0,
    "Read the validation paths\nEditing src/validation.rs\nAdded focused error messages\nAdded regression coverage\nCompleted — all checks pass",
    now - 80,
  )?;
    root.db.replace_run_checks(
        winner.id,
        &[
            check(winner.id, "typecheck", "pass", "cargo check passed", 1420),
            check(winner.id, "lint", "pass", "cargo clippy passed", 2110),
            check(winner.id, "test", "pass", "12 tests passed", 3180),
        ],
    )?;
    root.db.add_annotation(
        winner.id,
        "src/validation.rs",
        15,
        store::Side::New,
        "Keep this wording consistent with the form label.",
        now - 45,
    )?;

    let blocked = root.db.create_run(
        task.id,
        "claude-code",
        &demo.runs[1].to_string_lossy(),
        "asylum/validation-claude",
    )?;
    root.db.start_run(blocked.id, now - 310)?;
    root.db.save_run_output(
    blocked.id,
    "Inspecting the form and API\nI found two possible compatibility paths.\nShould the API return one error or collect every invalid field? (y/n)\n❯",
  )?;
    root.db.set_run_activity(blocked.id, Some("blocked"))?;

    let alternate = root.db.create_run(
        task.id,
        "opencode",
        &demo.runs[2].to_string_lossy(),
        "asylum/validation-opencode",
    )?;
    root.db.start_run(alternate.id, now - 260)?;
    root.db.finish_run_with_output(
        alternate.id,
        0,
        "Updated validation messages\nAdded table-driven tests\nCompleted implementation",
        now - 55,
    )?;
    root.db.replace_run_checks(
        alternate.id,
        &[
            check(
                alternate.id,
                "typecheck",
                "pass",
                "cargo check passed",
                1330,
            ),
            check(alternate.id, "test", "fail", "1 test failed", 2860),
        ],
    )?;

    root.db
        .record_event("run_started", Some(task.id), Some(winner.id), "", now - 420)?;
    root.db.record_event(
        "control_spawn",
        Some(task.id),
        Some(winner.id),
        r#"{"agent":"opencode","prompt":"Write table-driven tests for validation edge cases."}"#,
        now - 280,
    )?;
    root.db.record_event(
        "run_activity",
        Some(task.id),
        Some(blocked.id),
        r#"{"activity":"blocked"}"#,
        now - 35,
    )?;
    root.db.notify(
        "attention",
        "Claude Code needs input",
        "The validation run is blocked on an API compatibility choice.",
        Some(blocked.id),
        now - 35,
    )?;

    root.prs = vec![github::PullRequest {
        number: 42,
        title: "Make validation errors actionable".into(),
        author: "wess".into(),
        state: "OPEN".into(),
        head: "asylum/validation-codex".into(),
        base: "main".into(),
        draft: false,
        url: "https://example.invalid/pull/42".into(),
    }];
    root.issues = vec![
        github::Issue {
            number: 118,
            title: "Explain validation failures beside each field".into(),
            author: "aria".into(),
            state: "OPEN".into(),
            labels: vec!["accessibility".into(), "good first task".into()],
            url: "https://example.invalid/issues/118".into(),
        },
        github::Issue {
            number: 104,
            title: "Add keyboard coverage for the account form".into(),
            author: "devon".into(),
            state: "OPEN".into(),
            labels: vec!["tests".into()],
            url: "https://example.invalid/issues/104".into(),
        },
    ];

    let account = root.db.add_account("codex", "work", now - 600)?;
    root.db
        .record_usage(account.id, 38, Some(100), Some(now + 7200), now - 30)?;

    root.project_id = Some(project.id);
    root.task_id = Some(task.id);
    root.selected_run_id = Some(winner.id);
    root.fanout = vec!["codex".into(), "claude-code".into()];
    root.setup_open = false;
    root.refresh_setup();
    Ok(())
}

fn check(run_id: i64, id: &str, status: &str, summary: &str, duration_ms: u64) -> store::RunCheck {
    store::RunCheck {
        run_id,
        id: id.into(),
        status: status.into(),
        summary: summary.into(),
        duration_ms,
    }
}

fn demorepo() -> anyhow::Result<DemoRepo> {
    let root = std::env::temp_dir().join(format!("asylumsite-demo-{}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root)?;
    }
    std::fs::create_dir_all(root.join("src"))?;
    std::fs::create_dir_all(root.join("tests"))?;
    std::fs::write(
    root.join("src/validation.rs"),
    "pub fn validate(name: &str, email: &str) -> Result<(), String> {\n  if name.trim().is_empty() || !email.contains('@') {\n    return Err(\"invalid input\".into());\n  }\n  Ok(())\n}\n",
  )?;
    std::fs::write(
    root.join("tests/validation.rs"),
    "#[test]\nfn accepts_valid_input() {\n  assert!(validate(\"Sam\", \"sam@example.com\").is_ok());\n}\n",
  )?;
    git(&root, &["init", "-b", "main"])?;
    git(&root, &["config", "user.name", "Asylum Demo"])?;
    git(&root, &["config", "user.email", "demo@asylum.invalid"])?;
    git(&root, &["add", "."])?;
    git(&root, &["commit", "-m", "Add validation example"])?;

    let worktrees = root.join("worktrees");
    std::fs::create_dir_all(&worktrees)?;
    let specs = [
        ("codex", "asylum/validation-codex"),
        ("claude", "asylum/validation-claude"),
        ("opencode", "asylum/validation-opencode"),
    ];
    let mut runs = Vec::new();
    for (name, branch) in specs {
        let path = worktrees.join(name);
        git(
            &root,
            &[
                "worktree",
                "add",
                "-b",
                branch,
                path.to_string_lossy().as_ref(),
            ],
        )?;
        runs.push(path);
    }

    std::fs::write(
    runs[0].join("src/validation.rs"),
    "pub fn validate(name: &str, email: &str) -> Result<(), String> {\n  if name.trim().is_empty() {\n    return Err(\"Name is required\".into());\n  }\n  if !email.contains('@') {\n    return Err(\"Email must include an @ sign\".into());\n  }\n  Ok(())\n}\n",
  )?;
    std::fs::write(
    runs[0].join("tests/validation.rs"),
    "#[test]\nfn explains_an_empty_name() {\n  assert_eq!(validate(\"\", \"sam@example.com\").unwrap_err(), \"Name is required\");\n}\n\n#[test]\nfn explains_a_malformed_email() {\n  assert!(validate(\"Sam\", \"sam.example.com\").unwrap_err().contains('@'));\n}\n",
  )?;
    std::fs::write(
    runs[1].join("src/validation.rs"),
    "pub fn validate(name: &str, email: &str) -> Result<(), Vec<String>> {\n  let mut errors = Vec::new();\n  if name.trim().is_empty() { errors.push(\"Name is required\".into()); }\n  if !email.contains('@') { errors.push(\"Email is invalid\".into()); }\n  if errors.is_empty() { Ok(()) } else { Err(errors) }\n}\n",
  )?;
    std::fs::write(
    runs[2].join("src/validation.rs"),
    "pub fn validate(name: &str, email: &str) -> Result<(), String> {\n  match (name.trim().is_empty(), email.contains('@')) {\n    (true, _) => Err(\"Please enter a name\".into()),\n    (_, false) => Err(\"Please enter a valid email\".into()),\n    _ => Ok(()),\n  }\n}\n",
  )?;
    Ok(DemoRepo { root, runs })
}

fn git(dir: &Path, args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .with_context(|| format!("run git {}", args.join(" ")))?;
    if output.status.success() {
        return Ok(());
    }
    anyhow::bail!(
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    )
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
