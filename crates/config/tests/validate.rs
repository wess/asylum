use super::*;
use crate::model::{CustomAgent, Layout, Settings};
use crate::project::ProjectConfig;

fn temp_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("asylumvalidate{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir.join(name)
}

// ── Defaults stay clean ──────────────────────────────────────────────────────

#[test]
fn defaults_validate_clean() {
    let mut settings = Settings::default();
    assert!(validate(&mut settings).is_empty());
    // And nothing was mutated along the way.
    assert_eq!(settings, Settings::default());
}

// ── Server binds ─────────────────────────────────────────────────────────────

#[test]
fn unparseable_bind_is_flagged_and_kept() {
    let mut settings = Settings::default();
    settings.companion.bind = "not-a-bind-address".to_string();
    let diags = validate(&mut settings);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].key, "companion.bind");
    assert!(
        diags[0].message.contains("companion.bind"),
        "{}",
        diags[0].message
    );
    // Warned, not overwritten.
    assert_eq!(settings.companion.bind, "not-a-bind-address");
}

#[test]
fn bind_missing_a_port_is_flagged() {
    let mut settings = Settings::default();
    settings.control.bind = "127.0.0.1".to_string();
    let diags = validate(&mut settings);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].key, "control.bind");
}

#[test]
fn hostname_bind_is_not_flagged() {
    // A bare hostname does not parse as a `SocketAddr`, but it is resolved
    // (with DNS) at actual startup by `bind::guard` - validation only checks
    // shape, so it must not be flagged here.
    let mut settings = Settings::default();
    settings.mcp.bind = "localhost:8790".to_string();
    assert!(validate(&mut settings).is_empty());
}

#[test]
fn colliding_ports_are_flagged_on_both_keys() {
    let mut settings = Settings::default();
    settings.control.bind = "127.0.0.1:8787".to_string(); // same port as companion's default
    let diags = validate(&mut settings);
    let keys: Vec<&str> = diags.iter().map(|d| d.key.as_str()).collect();
    assert!(keys.contains(&"companion.bind"), "{keys:?}");
    assert!(keys.contains(&"control.bind"), "{keys:?}");
    for d in &diags {
        assert!(d.message.contains("8787"), "{}", d.message);
    }
    // Values are left as configured; there is no way to know which one is wrong.
    assert_eq!(settings.control.bind, "127.0.0.1:8787");
}

#[test]
fn zero_port_binds_never_collide() {
    let mut settings = Settings::default();
    settings.companion.bind = "127.0.0.1:0".to_string();
    settings.control.bind = "127.0.0.1:0".to_string();
    assert!(validate(&mut settings).is_empty());
}

// ── worktree_dir ─────────────────────────────────────────────────────────────

#[test]
fn empty_worktree_dir_is_flagged_and_defaulted() {
    let mut settings = Settings {
        worktree_dir: "".to_string(),
        ..Default::default()
    };
    let diags = validate(&mut settings);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].key, "worktree_dir");
    assert_eq!(settings.worktree_dir, Settings::default().worktree_dir);
}

#[test]
fn tilde_worktree_dir_is_flagged_but_kept() {
    let mut settings = Settings {
        worktree_dir: "~/asylum-worktrees".to_string(),
        ..Default::default()
    };
    let diags = validate(&mut settings);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].key, "worktree_dir");
    assert!(
        diags[0].message.contains("worktree_dir"),
        "{}",
        diags[0].message
    );
    assert_eq!(settings.worktree_dir, "~/asylum-worktrees");
}

#[test]
fn backslash_worktree_dir_is_flagged() {
    let mut settings = Settings {
        worktree_dir: "worktrees\\tmp".to_string(),
        ..Default::default()
    };
    let diags = validate(&mut settings);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].key, "worktree_dir");
}

#[test]
fn worktree_dir_pointing_at_a_file_is_flagged_but_kept() {
    let file = temp_path("blocks-worktree-dir.txt");
    std::fs::write(&file, "occupied").unwrap();
    let mut settings = Settings {
        worktree_dir: file.to_string_lossy().into_owned(),
        ..Default::default()
    };
    let diags = validate(&mut settings);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].key, "worktree_dir");
    assert!(
        diags[0].message.contains("existing file"),
        "{}",
        diags[0].message
    );
    // Kept as-is; only a warning.
    assert_eq!(settings.worktree_dir, file.to_string_lossy());
    let _ = std::fs::remove_file(&file);
}

#[test]
fn relative_worktree_dir_skips_the_file_probe() {
    // Relative to a project root this crate does not know; must not be
    // checked against the process's current directory.
    let mut settings = Settings {
        // Exists relative to this crate, and is a file, but is not absolute.
        worktree_dir: "Cargo.toml".to_string(),
        ..Default::default()
    };
    assert!(validate(&mut settings).is_empty());
}

// ── Concurrency / timeout numerics ──────────────────────────────────────────

#[test]
fn zero_max_parallel_runs_is_the_unlimited_sentinel_and_clean() {
    let mut settings = Settings {
        max_parallel_runs: 0,
        ..Default::default()
    };
    assert!(validate(&mut settings).is_empty());
    assert_eq!(settings.max_parallel_runs, 0);
}

#[test]
fn absurd_max_parallel_runs_is_flagged_and_bounded() {
    let mut settings = Settings {
        max_parallel_runs: 5_000,
        ..Default::default()
    };
    let diags = validate(&mut settings);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].key, "max_parallel_runs");
    assert_eq!(
        settings.max_parallel_runs,
        Settings::default().max_parallel_runs
    );
}

#[test]
fn zero_run_timeout_is_the_no_timeout_sentinel_and_clean() {
    let mut settings = Settings {
        run_timeout_minutes: 0,
        ..Default::default()
    };
    assert!(validate(&mut settings).is_empty());
}

#[test]
fn absurd_run_timeout_is_flagged_and_bounded() {
    let mut settings = Settings {
        run_timeout_minutes: 999_999,
        ..Default::default()
    };
    let diags = validate(&mut settings);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].key, "run_timeout_minutes");
    assert_eq!(
        settings.run_timeout_minutes,
        Settings::default().run_timeout_minutes
    );
}

// ── Layouts ──────────────────────────────────────────────────────────────────

#[test]
fn layout_with_unknown_agent_id_is_flagged_but_kept() {
    let mut settings = Settings {
        layouts: vec![Layout {
            name: "solo".to_string(),
            description: String::new(),
            agents: vec!["definitely-not-a-real-agent".to_string()],
            concurrency: 0,
        }],
        ..Default::default()
    };
    let diags = validate(&mut settings);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].key, "layouts[0].agents");
    assert!(
        diags[0].message.contains("definitely-not-a-real-agent"),
        "{}",
        diags[0].message
    );
    // The layout is not mutated; the id is kept in place.
    assert_eq!(
        settings.layouts[0].agents,
        vec!["definitely-not-a-real-agent"]
    );
}

#[test]
fn layout_referencing_a_custom_agent_is_clean() {
    let mut settings = Settings {
        custom_agents: vec![CustomAgent {
            id: "my-wrapper".to_string(),
            name: "My Wrapper".to_string(),
            icon: String::new(),
            program: "my-wrapper".to_string(),
            args: Vec::new(),
            delivery: "arg".to_string(),
        }],
        layouts: vec![Layout {
            name: "solo".to_string(),
            description: String::new(),
            agents: vec!["my-wrapper".to_string()],
            concurrency: 0,
        }],
        ..Default::default()
    };
    assert!(validate(&mut settings).is_empty());
}

#[test]
fn every_builtin_registry_id_is_known_to_validation() {
    // `Layout::builtins()` already exercises a handful of these; every id it
    // uses must be present in the mirrored list or the default layouts
    // themselves would fail `defaults_validate_clean`. This locks in that the
    // mirrored set stays a superset of what ships.
    for layout in Layout::builtins() {
        for id in &layout.agents {
            assert!(
                KNOWN_BUILTIN_AGENT_IDS.contains(&id.as_str()),
                "builtin layout '{}' uses unmirrored id '{id}'",
                layout.name
            );
        }
    }
}

#[test]
fn layout_concurrency_zero_defers_to_global_and_is_clean() {
    let mut settings = Settings {
        layouts: vec![Layout {
            name: "solo".to_string(),
            description: String::new(),
            agents: vec!["claude-code".to_string()],
            concurrency: 0,
        }],
        ..Default::default()
    };
    assert!(validate(&mut settings).is_empty());
}

#[test]
fn absurd_layout_concurrency_is_flagged_and_reset_to_zero() {
    let mut settings = Settings {
        layouts: vec![Layout {
            name: "solo".to_string(),
            description: String::new(),
            agents: vec!["claude-code".to_string()],
            concurrency: 100_000,
        }],
        ..Default::default()
    };
    let diags = validate(&mut settings);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].key, "layouts[0].concurrency");
    assert_eq!(settings.layouts[0].concurrency, 0);
}

#[test]
fn unnamed_layout_is_labeled_by_index() {
    let mut settings = Settings {
        layouts: vec![Layout {
            name: String::new(),
            description: String::new(),
            agents: vec!["nope".to_string()],
            concurrency: 0,
        }],
        ..Default::default()
    };
    let diags = validate(&mut settings);
    assert_eq!(diags.len(), 1);
    assert!(diags[0].message.contains("#0"), "{}", diags[0].message);
}

// ── Custom agents ────────────────────────────────────────────────────────────

#[test]
fn custom_agent_with_empty_program_is_flagged_but_kept() {
    let mut settings = Settings {
        custom_agents: vec![CustomAgent {
            id: "ghost".to_string(),
            name: String::new(),
            icon: String::new(),
            program: "".to_string(),
            args: Vec::new(),
            delivery: "arg".to_string(),
        }],
        ..Default::default()
    };
    let diags = validate(&mut settings);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].key, "custom_agents[0].program");
    assert!(diags[0].message.contains("ghost"), "{}", diags[0].message);
    assert_eq!(settings.custom_agents[0].program, "");
}

#[test]
fn custom_agent_with_empty_id_is_flagged() {
    let mut settings = Settings {
        custom_agents: vec![CustomAgent {
            id: "".to_string(),
            name: String::new(),
            icon: String::new(),
            program: "somebin".to_string(),
            args: Vec::new(),
            delivery: "arg".to_string(),
        }],
        ..Default::default()
    };
    let diags = validate(&mut settings);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].key, "custom_agents[0].id");
}

#[test]
fn well_formed_custom_agent_is_clean() {
    let mut settings = Settings {
        custom_agents: vec![CustomAgent {
            id: "my-agent".to_string(),
            name: "My Agent".to_string(),
            icon: String::new(),
            program: "my-agent-bin".to_string(),
            args: Vec::new(),
            delivery: "arg".to_string(),
        }],
        ..Default::default()
    };
    assert!(validate(&mut settings).is_empty());
}

// ── base_branch (ProjectConfig) ──────────────────────────────────────────────

#[test]
fn absent_base_branch_is_clean() {
    let mut cfg = ProjectConfig::default();
    assert!(validate_project(&mut cfg).is_empty());
    assert_eq!(cfg.base_branch, None);
}

#[test]
fn good_base_branch_is_clean() {
    let mut cfg = ProjectConfig {
        base_branch: Some("main".to_string()),
        ..Default::default()
    };
    assert!(validate_project(&mut cfg).is_empty());
    assert_eq!(cfg.base_branch.as_deref(), Some("main"));
}

#[test]
fn empty_base_branch_is_flagged_and_cleared() {
    let mut cfg = ProjectConfig {
        base_branch: Some("".to_string()),
        ..Default::default()
    };
    let diags = validate_project(&mut cfg);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].key, "base_branch");
    assert_eq!(cfg.base_branch, None);
}

#[test]
fn base_branch_with_forbidden_characters_is_flagged_and_cleared() {
    for bad in [
        "feature..branch",
        "bad~branch",
        "trailing/",
        "foo bar",
        "a//b",
        "x@{y",
    ] {
        let mut cfg = ProjectConfig {
            base_branch: Some(bad.to_string()),
            ..Default::default()
        };
        let diags = validate_project(&mut cfg);
        assert_eq!(diags.len(), 1, "branch: {bad}");
        assert_eq!(diags[0].key, "base_branch");
        assert!(
            diags[0].message.contains("base_branch"),
            "{}",
            diags[0].message
        );
        assert_eq!(cfg.base_branch, None, "branch: {bad}");
    }
}

#[test]
fn base_branch_named_at_sign_is_flagged() {
    let mut cfg = ProjectConfig {
        base_branch: Some("@".to_string()),
        ..Default::default()
    };
    let diags = validate_project(&mut cfg);
    assert_eq!(diags.len(), 1);
    assert_eq!(cfg.base_branch, None);
}
