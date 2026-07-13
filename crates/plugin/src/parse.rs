//! TOML manifest parsing: `plugin.toml` text → [`Plugin`].
//!
//! We deserialize into private `Raw*` structs that mirror the TOML shape, then
//! validate and lower into the public [`model`](crate::model) types. Lowering is
//! where tokens (`kind = "wasm"`, `placement = "tab"`, …) are checked and
//! unknown values become an error string.

use std::path::PathBuf;

use serde::Deserialize;

use crate::model::*;

/// Parse a `plugin.toml` string. `dir` is the plugin directory, stamped into
/// the returned [`Plugin::path`]. Returns a human-readable error on any problem.
pub fn parse(text: &str, dir: PathBuf) -> Result<Plugin, String> {
    let raw: RawPlugin = toml::from_str(text).map_err(|e| e.message().to_string())?;

    if raw.id.trim().is_empty() {
        return Err("missing `id`".into());
    }
    if raw.name.trim().is_empty() {
        return Err("missing `name`".into());
    }

    for cap in &raw.capabilities {
        if !CAPABILITIES.contains(&cap.as_str()) {
            return Err(format!("unknown capability `{cap}`"));
        }
    }

    let runtime = raw.runtime.map(lower_runtime).transpose()?;
    let commands = raw
        .command
        .into_iter()
        .map(lower_command)
        .collect::<Result<Vec<_>, _>>()?;
    let panel = raw.panel.map(lower_panel);
    let webview = raw.webview.map(lower_webview).transpose()?;
    let triggers = raw
        .trigger
        .into_iter()
        .map(lower_trigger)
        .collect::<Result<Vec<_>, _>>()?;
    let tools = raw.tool.into_iter().map(lower_tool).collect();

    Ok(Plugin {
        id: raw.id,
        name: raw.name,
        version: raw.version.unwrap_or_else(|| "0.0.0".into()),
        description: raw.description,
        path: dir,
        commands,
        runtime,
        panel,
        webview,
        triggers,
        tools,
        capabilities: raw.capabilities,
    })
}

fn lower_runtime(r: RawRuntime) -> Result<Runtime, String> {
    let kind = RuntimeKind::parse(r.kind.as_deref().unwrap_or("process"))
        .ok_or_else(|| format!("unknown runtime type `{}`", r.kind.unwrap_or_default()))?;
    if kind == RuntimeKind::Process && r.command.as_deref().unwrap_or("").is_empty() {
        return Err("`[runtime]` of type process needs a `command`".into());
    }
    if kind == RuntimeKind::Wasm && r.wasm.as_deref().unwrap_or("").is_empty() {
        return Err("`[runtime]` of type wasm needs a `wasm` path".into());
    }
    Ok(Runtime {
        kind,
        command: r.command.unwrap_or_default(),
        wasm: r.wasm,
        persistent: r.persistent.unwrap_or(false),
    })
}

fn lower_command(c: RawCommand) -> Result<Command, String> {
    if c.id.trim().is_empty() {
        return Err("command missing `id`".into());
    }
    let mode = CommandMode::parse(c.mode.as_deref().unwrap_or(""))
        .ok_or_else(|| format!("command `{}` has unknown mode", c.id))?;
    Ok(Command {
        title: c.title.unwrap_or_else(|| c.id.clone()),
        run: c.run.unwrap_or_default(),
        mode,
        keybind: c.keybind,
        id: c.id,
    })
}

fn lower_panel(p: RawPanel) -> Panel {
    Panel {
        id: p.id.clone(),
        title: p.title.unwrap_or(p.id),
        icon: p.icon.unwrap_or_else(|| "▣".into()),
    }
}

fn lower_webview(w: RawWebview) -> Result<Webview, String> {
    let placement = Placement::parse(w.placement.as_deref().unwrap_or("panel"))
        .ok_or_else(|| format!("webview `{}` has unknown placement", w.id))?;
    let source = match (w.url, w.entry, w.service) {
        (Some(u), _, _) => WebviewSource::Url(u),
        (_, Some(e), _) => WebviewSource::Entry(e),
        (_, _, Some(s)) => WebviewSource::Service(s),
        _ => return Err(format!("webview `{}` needs a url, entry, or service", w.id)),
    };
    Ok(Webview {
        title: w.title.unwrap_or_else(|| w.id.clone()),
        icon: w.icon.unwrap_or_else(|| "◲".into()),
        id: w.id,
        placement,
        source,
    })
}

fn lower_trigger(t: RawTrigger) -> Result<Trigger, String> {
    if !TRIGGER_EVENTS.contains(&t.on.as_str()) {
        return Err(format!("unknown trigger event `{}`", t.on));
    }
    let action = match (t.notify, t.invoke) {
        (Some(text), _) => TriggerAction::Notify { text },
        (_, Some(method)) => TriggerAction::Invoke { method },
        _ => return Err(format!("trigger on `{}` needs `notify` or `invoke`", t.on)),
    };
    Ok(Trigger {
        on: t.on,
        when: t.when,
        action,
    })
}

fn lower_tool(t: RawTool) -> Tool {
    let params = t
        .param
        .into_iter()
        .map(|p| ToolParam {
            name: p.name,
            kind: p.kind.unwrap_or_else(|| "string".into()),
            description: p.description.unwrap_or_default(),
            required: p.required.unwrap_or(false),
        })
        .collect();
    Tool {
        id: t.id,
        description: t.description.unwrap_or_default(),
        params,
    }
}

// ── Raw TOML shapes ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RawPlugin {
    id: String,
    name: String,
    version: Option<String>,
    description: Option<String>,
    #[serde(default)]
    capabilities: Vec<String>,
    runtime: Option<RawRuntime>,
    panel: Option<RawPanel>,
    webview: Option<RawWebview>,
    #[serde(default)]
    command: Vec<RawCommand>,
    #[serde(default)]
    trigger: Vec<RawTrigger>,
    #[serde(default)]
    tool: Vec<RawTool>,
}

#[derive(Deserialize)]
struct RawRuntime {
    #[serde(rename = "type")]
    kind: Option<String>,
    command: Option<String>,
    wasm: Option<String>,
    persistent: Option<bool>,
}

#[derive(Deserialize)]
struct RawPanel {
    id: String,
    title: Option<String>,
    icon: Option<String>,
}

#[derive(Deserialize)]
struct RawWebview {
    id: String,
    title: Option<String>,
    icon: Option<String>,
    placement: Option<String>,
    url: Option<String>,
    entry: Option<String>,
    service: Option<String>,
}

#[derive(Deserialize)]
struct RawCommand {
    id: String,
    title: Option<String>,
    run: Option<String>,
    mode: Option<String>,
    keybind: Option<String>,
}

#[derive(Deserialize)]
struct RawTrigger {
    on: String,
    when: Option<String>,
    notify: Option<String>,
    invoke: Option<String>,
}

#[derive(Deserialize)]
struct RawTool {
    id: String,
    description: Option<String>,
    #[serde(default)]
    param: Vec<RawToolParam>,
}

#[derive(Deserialize)]
struct RawToolParam {
    name: String,
    #[serde(rename = "type")]
    kind: Option<String>,
    description: Option<String>,
    required: Option<bool>,
}

#[cfg(test)]
#[path = "../tests/parse.rs"]
mod tests;
