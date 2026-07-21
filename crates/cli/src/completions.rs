//! `asylum completions <bash|zsh|fish>` - static shell completion scripts for
//! the subcommand tree.
//!
//! The top-level and nested subcommand names are pulled from [`help::TOPICS`]
//! at print time, so the completion tree can't drift from the help table -
//! see `tests/completions.rs` for the coverage check. Everything else
//! (install hints, the case/switch skeleton, file completion for path-taking
//! flags) is a hand-authored template; that's appropriate at this scale and
//! keeps this file dependency-free.

use crate::help;

pub fn run(args: &[String]) -> Result<(), String> {
    let shell = crate::positionals(args).first().cloned().ok_or_else(|| {
        format!(
            "usage: asylum completions <bash|zsh|fish> {}",
            help::hint(&["completions"])
        )
    })?;
    let script = match shell.as_str() {
        "bash" => bash_script(),
        "zsh" => zsh_script(),
        "fish" => fish_script(),
        other => {
            return Err(format!(
                "unsupported shell `{other}` (want bash, zsh, or fish) {}",
                help::hint(&["completions"])
            ))
        }
    };
    print!("{script}");
    Ok(())
}

/// Top-level command names, in table order.
fn top_level_names() -> Vec<&'static str> {
    help::TOPICS
        .iter()
        .filter(|t| t.path.len() == 1)
        .map(|t| t.path[0])
        .collect()
}

/// Documented nested subcommand names under `top`, in table order.
fn nested_names(top: &str) -> Vec<&'static str> {
    help::TOPICS
        .iter()
        .filter(|t| t.path.len() == 2 && t.path[0] == top)
        .map(|t| t.path[1])
        .collect()
}

/// Flags across the CLI whose value is a directory path, so shells can offer
/// directory completion instead of an opaque word.
const DIR_FLAGS: &[&str] = &["--repo", "--cwd", "--dir", "--start"];

fn bash_script() -> String {
    let mut s = String::new();
    s.push_str("# asylum bash completion\n");
    s.push_str("# install: source this file from ~/.bashrc or ~/.bash_profile, or drop it in\n");
    s.push_str(
        "# your bash-completion directory - Homebrew: $(brew --prefix)/etc/bash_completion.d,\n",
    );
    s.push_str("# Debian/Ubuntu: /etc/bash_completion.d - as `asylum`.\n\n");
    s.push_str("_asylum() {\n");
    s.push_str("    local cur prev top sub\n");
    s.push_str("    cur=\"${COMP_WORDS[COMP_CWORD]}\"\n");
    s.push_str("    prev=\"${COMP_WORDS[COMP_CWORD-1]}\"\n");
    s.push_str(&format!("    top=\"{}\"\n\n", top_level_names().join(" ")));

    s.push_str("    if [ \"$COMP_CWORD\" -eq 1 ]; then\n");
    s.push_str("        COMPREPLY=($(compgen -W \"$top\" -- \"$cur\"))\n");
    s.push_str("        return\n");
    s.push_str("    fi\n\n");

    s.push_str("    case \"$prev\" in\n");
    s.push_str(&format!("        {})\n", DIR_FLAGS.join("|")));
    s.push_str("            COMPREPLY=($(compgen -d -- \"$cur\"))\n");
    s.push_str("            return\n");
    s.push_str("            ;;\n");
    s.push_str("    esac\n\n");

    s.push_str("    case \"${COMP_WORDS[1]}\" in\n");
    for top in top_level_names() {
        let subs = nested_names(top);
        if subs.is_empty() {
            continue;
        }
        s.push_str(&format!("        {top}) sub=\"{}\" ;;\n", subs.join(" ")));
    }
    s.push_str("        *) sub=\"\" ;;\n");
    s.push_str("    esac\n\n");

    s.push_str("    if [ \"$COMP_CWORD\" -eq 2 ] && [ -n \"$sub\" ]; then\n");
    s.push_str("        COMPREPLY=($(compgen -W \"$sub\" -- \"$cur\"))\n");
    s.push_str("        return\n");
    s.push_str("    fi\n\n");

    s.push_str("    COMPREPLY=($(compgen -f -- \"$cur\"))\n");
    s.push_str("}\n");
    s.push_str("complete -F _asylum asylum\n");
    s
}

fn zsh_script() -> String {
    let mut s = String::new();
    s.push_str("#compdef asylum\n");
    s.push_str("# asylum zsh completion\n");
    s.push_str("# install: save this file as `_asylum` in a directory on your $fpath\n");
    s.push_str("# (e.g. ~/.zsh/completions, added via `fpath+=(~/.zsh/completions)` before\n");
    s.push_str(
        "# `compinit` runs in your ~/.zshrc), then restart your shell or run `compinit`.\n\n",
    );
    s.push_str("_asylum() {\n");
    s.push_str("    local prev sub\n");
    s.push_str("    prev=\"${words[CURRENT-1]}\"\n");
    s.push_str(&format!(
        "    local -a top; top=({})\n\n",
        top_level_names().join(" ")
    ));

    s.push_str("    if (( CURRENT == 2 )); then\n");
    s.push_str("        compadd -- ${top[@]}\n");
    s.push_str("        return\n");
    s.push_str("    fi\n\n");

    s.push_str("    case \"$prev\" in\n");
    s.push_str(&format!("        {})\n", DIR_FLAGS.join("|")));
    s.push_str("            _files -/\n");
    s.push_str("            return\n");
    s.push_str("            ;;\n");
    s.push_str("    esac\n\n");

    s.push_str("    case \"${words[2]}\" in\n");
    for top in top_level_names() {
        let subs = nested_names(top);
        if subs.is_empty() {
            continue;
        }
        s.push_str(&format!("        {top}) sub=\"{}\" ;;\n", subs.join(" ")));
    }
    s.push_str("        *) sub=\"\" ;;\n");
    s.push_str("    esac\n\n");

    s.push_str("    if (( CURRENT == 3 )) && [[ -n \"$sub\" ]]; then\n");
    s.push_str("        compadd -- ${=sub}\n");
    s.push_str("        return\n");
    s.push_str("    fi\n\n");

    s.push_str("    _files\n");
    s.push_str("}\n\n");
    s.push_str("compdef _asylum asylum\n");
    s
}

fn fish_script() -> String {
    let mut s = String::new();
    s.push_str("# asylum fish completion\n");
    s.push_str("# install: save this file as ~/.config/fish/completions/asylum.fish (or, for\n");
    s.push_str("# all users, /usr/local/share/fish/vendor_completions.d/asylum.fish); fish\n");
    s.push_str("# loads completions from those directories automatically in new shells.\n\n");

    s.push_str("complete -c asylum -f\n\n");

    let top = top_level_names().join(" ");
    s.push_str(&format!("set -l asylum_top {top}\n"));
    s.push_str("complete -c asylum -n \"not __fish_seen_subcommand_from $asylum_top\" -a \"$asylum_top\"\n\n");

    for name in top_level_names() {
        let subs = nested_names(name);
        if subs.is_empty() {
            continue;
        }
        s.push_str(&format!(
            "complete -c asylum -n \"__fish_seen_subcommand_from {name}\" -a \"{}\"\n",
            subs.join(" ")
        ));
    }
    s.push('\n');

    for flag in DIR_FLAGS {
        s.push_str(&format!(
            "complete -c asylum -l {} -rxa \"(__fish_complete_directories)\"\n",
            flag.trim_start_matches('-')
        ));
    }
    s
}

#[cfg(test)]
#[path = "../tests/completions.rs"]
mod tests;
