use super::*;

#[test]
fn every_script_contains_every_top_level_command() {
    let scripts = [
        ("bash", bash_script()),
        ("zsh", zsh_script()),
        ("fish", fish_script()),
    ];
    for name in top_level_names() {
        for (shell, script) in &scripts {
            assert!(
                script.contains(name),
                "{shell} completion is missing top-level command `{name}`"
            );
        }
    }
}

#[test]
fn every_script_contains_every_documented_nested_subcommand() {
    let scripts = [bash_script(), zsh_script(), fish_script()];
    for top in top_level_names() {
        for sub in nested_names(top) {
            for script in &scripts {
                assert!(
                    script.contains(sub),
                    "a completion script for `{top}` is missing nested subcommand `{sub}`"
                );
            }
        }
    }
}

#[test]
fn scripts_carry_install_hints() {
    assert!(bash_script().contains("bashrc") || bash_script().contains("bash_completion"));
    assert!(zsh_script().contains("fpath"));
    assert!(fish_script().contains("completions/asylum.fish"));
}

#[test]
fn scripts_offer_directory_completion_for_path_flags() {
    assert!(bash_script().contains("compgen -d"));
    assert!(zsh_script().contains("_files -/"));
    assert!(fish_script().contains("__fish_complete_directories"));
}

#[test]
fn run_prints_a_script_for_each_supported_shell() {
    for shell in ["bash", "zsh", "fish"] {
        assert!(run(&[shell.to_string()]).is_ok());
    }
}

#[test]
fn run_rejects_an_unknown_shell() {
    let err = run(&["powershell".to_string()]).unwrap_err();
    assert!(err.contains("unsupported shell"));
    assert!(err.contains("completions --help"));
}

#[test]
fn run_requires_a_shell_argument() {
    let err = run(&[]).unwrap_err();
    assert!(err.contains("usage"));
}
