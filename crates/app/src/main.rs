//! Asylum - an Agent Development Environment.
//!
//! `main` loads settings, installs the guise theme, wires the native menu bar,
//! and opens the root window. The heavy lifting lives in the domain crates
//! (`store`, `agent`, `git`, `plugin`) and the view modules here.

mod accounts;
mod browser;
mod control;
mod crash;
mod diff;
mod fleet;
mod icons;
mod integrations;
mod logs;
mod menu;
mod menus;
mod note;
mod notifications;
mod plugins;
mod prepare;
mod reap;
mod reload;
mod root;
mod run;
mod search;
mod secrets;
mod settings;
mod setup;
mod sidebar;
#[cfg(feature = "sitecapture")]
mod sitecapture;
mod state;
mod theme;
mod workspace;

use gpui::AppContext as _;
use gpui::{point, px, size, App, Bounds, TitlebarOptions, WindowBounds, WindowOptions};
use zeroize::Zeroizing;

use state::Root;

fn main() {
    // Version fast-path: checked before logging, the sitecapture branch, the
    // instance lock, or anything else, so `asylum --version` is a cheap,
    // headless-safe check (no gpui/display init) that CI can run on every
    // platform.
    if std::env::args()
        .skip(1)
        .any(|arg| matches!(arg.as_str(), "--version" | "-V" | "version"))
    {
        println!("asylum {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    // Logging comes up before everything else, including the single-instance
    // lock, so a refusal or an early failure still lands in the log file.
    // `logs::init` creates `<data dir>/logs` itself (via `RollingFileAppender`),
    // so nothing here needs the data dir to already exist.
    let db_path = state::Root::db_path();
    let _log_guard = logs::init();
    crash::install();
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        data_dir = %db_path.parent().unwrap_or(std::path::Path::new(".")).display(),
        "asylum starting"
    );

    // Settings drive the initial theme; a missing file is fine (defaults).
    // Diagnostics are reported when the load is applied (see `reload`).
    let loaded = config::load(&config::default_path());

    #[cfg(feature = "sitecapture")]
    if let Some(path) = std::env::var_os("ASYLUM_SITE_CAPTURE") {
        sitecapture::run(&loaded.settings, path.into()).expect("capture Asylum window");
        return;
    }

    // Single-instance guard: take an exclusive advisory lock beside the database
    // before opening the store, running interrupted-run recovery, or starting
    // any server. A second instance would re-run recovery - marking this
    // instance's live runs failed - and both would drain the followup/control
    // queues and double-launch queued work, so a contended lock is a hard
    // refusal, not a warning. `_instance` holds the lock for the whole process
    // lifetime; the OS releases it on exit (no stale lock to clear).
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _instance = match store::lock::acquire(&store::lock::path_for(&db_path)) {
        Ok(Some(guard)) => guard,
        Ok(None) => {
            tracing::warn!("refusing to start: another instance holds the workspace lock");
            eprintln!(
                "Asylum is already running. Another instance is using this workspace; quit it before relaunching."
            );
            let _ = notify::send(&notify::Notification::new(
                "Asylum is already running",
                "Another instance is using this workspace; quit it before relaunching.",
            ));
            std::process::exit(1);
        }
        Err(error) => {
            tracing::error!(%error, "could not acquire the single-instance lock");
            eprintln!("Asylum could not acquire its single-instance lock: {error}");
            let _ = notify::send(&notify::Notification::new(
                "Asylum could not start",
                format!("Could not acquire the single-instance lock: {error}"),
            ));
            std::process::exit(1);
        }
    };

    // Read + scrub the keep passphrase while still single-threaded (before any
    // thread starts), so it never lingers in `/proc/<pid>/environ`.
    //
    // The scrub is unconditional, and must stay that way: agents inherit this
    // process's environment, so a passphrase left set when the proxy is off
    // would hand every spawned agent the keys to the whole keep (`asylum keep
    // list` would print it). The proxy defaults to off, so the branch that
    // skipped the scrub was the common one.
    let passphrase = std::env::var("ASYLUM_KEEP_PASSPHRASE")
        .ok()
        .filter(|p| !p.is_empty())
        .map(Zeroizing::new);
    std::env::remove_var("ASYLUM_KEEP_PASSPHRASE");

    // Once unlocked, secret values live only in memory (the shared keep handle)
    // and the proxy resolves from them; the values never reach an agent.
    let keep_handle: proxy::SharedKeep = std::sync::Arc::new(std::sync::Mutex::new(None));
    if loaded.settings.proxy.enabled {
        if let Some(pass) = passphrase.as_deref() {
            let path = keep_path();
            let opened = if path.exists() {
                keep::Keep::open(&path, pass)
            } else {
                keep::Keep::create(pass)
            };
            match opened {
                Ok(k) => *keep_handle.lock().unwrap() = Some(k),
                Err(e) => report_server_problem(
                    &db_path,
                    "Secrets keep locked",
                    &format!(
                        "Could not unlock the keep: {e}. Proxy calls will fail until unlocked."
                    ),
                ),
            }
        }
    }
    drop(passphrase);
    secrets::set_keep(keep_handle.clone());
    secrets::refresh_redaction_values();

    // Launch the mobile companion server on a background thread, serving the
    // same on-disk store the app uses. Bind, token, and enablement come from
    // settings; off by default. Once enabled it always requires a token, even
    // on loopback (it would otherwise let any local process read the fleet and
    // inject follow-ups into a live agent) - the refusal and any bind error are
    // surfaced in the Inbox rather than silently swallowed.
    let companion = loaded.settings.companion.clone();
    if companion.enabled {
        match config::bind::guard(
            &companion.bind,
            &companion.token,
            config::bind::Policy::TokenRequired,
        ) {
            Ok(()) => {
                tracing::info!(bind = %companion.bind, "companion server starting");
                let db_path = db_path.clone();
                let report_path = db_path.clone();
                std::thread::spawn(move || {
                    if let Err(error) =
                        companion::serve(db_path, companion.bind.as_str(), companion.token)
                    {
                        report_server_problem(
                            &report_path,
                            "Companion server stopped",
                            &format!("The mobile companion server failed: {error}"),
                        );
                    }
                });
            }
            Err(refusal) => {
                report_server_problem(&db_path, "Companion server disabled", &format!("{refusal}"))
            }
        }
    }

    // Launch the agent control surface on its own background thread. It shares
    // the same store and lets a running agent orchestrate the fleet (spawn a
    // helper run, read a sibling, report state, wait). `::control` disambiguates
    // the crate from this app's `control` UI module.
    //
    // Localhost is not an authentication boundary here - any local process could
    // otherwise spawn agents or read transcripts - so the control server always
    // runs with a token. When settings leave it empty we provision a strong
    // per-session token, kept in memory only (see `secrets`) and injected into
    // each managed agent. The bind is loopback-only.
    let control = loaded.settings.control.clone();
    if control.enabled {
        let token = if control.token.trim().is_empty() {
            match config::token::generate() {
                Ok(token) => token,
                Err(error) => {
                    report_server_problem(
                        &db_path,
                        "Control server disabled",
                        &format!("Could not generate a control token: {error}"),
                    );
                    String::new()
                }
            }
        } else {
            control.token.clone()
        };
        if !token.is_empty() {
            secrets::set_control_token(token.clone());
            match config::bind::guard(&control.bind, &token, config::bind::Policy::LoopbackOnly) {
                Ok(()) => {
                    tracing::info!(bind = %control.bind, "control server starting");
                    let db_path = db_path.clone();
                    let report_path = db_path.clone();
                    let bind = control.bind.clone();
                    std::thread::spawn(move || {
                        if let Err(error) = ::control::serve(db_path, bind.as_str(), token) {
                            report_server_problem(
                                &report_path,
                                "Control server stopped",
                                &format!("The agent control server failed: {error}"),
                            );
                        }
                    });
                }
                Err(refusal) => report_server_problem(
                    &db_path,
                    "Control server disabled",
                    &format!("{refusal}"),
                ),
            }
        }
    }

    // Launch the secrets proxy on its own thread: masked outbound API access for
    // agents. An agent calls a named upstream and the proxy resolves the secret
    // from the (per-project-scoped) keep and injects it server-side, so the key
    // is never exposed. Bind is loopback-only; each run gets a signed token
    // naming its project (minted from the session key).
    let proxy_prefs = loaded.settings.proxy.clone();
    if proxy_prefs.enabled {
        let key = config::token::generate().unwrap_or_default();
        if !key.is_empty() {
            secrets::set_proxy_key(key.clone());
            match config::bind::guard(&proxy_prefs.bind, &key, config::bind::Policy::LoopbackOnly) {
                Ok(()) => {
                    tracing::info!(bind = %proxy_prefs.bind, "secrets proxy starting");
                    let bind = proxy_prefs.bind.clone();
                    let upstreams = loaded.settings.upstreams.clone();
                    let report_path = db_path.clone();
                    let proxy = proxy::Proxy {
                        key,
                        upstreams,
                        keep: keep_handle.clone(),
                    };
                    std::thread::spawn(move || {
                        if let Err(error) = proxy::serve(bind.as_str(), proxy) {
                            report_server_problem(
                                &report_path,
                                "Secrets proxy stopped",
                                &format!("The secrets proxy failed: {error}"),
                            );
                        }
                    });
                }
                Err(refusal) => {
                    report_server_problem(&db_path, "Secrets proxy disabled", &format!("{refusal}"))
                }
            }
        }
    }

    // Launch the MCP gateway on its own thread: one aggregating MCP server every
    // agent connects to, fronting the configured upstream servers under
    // per-service namespaces (`<service>__<tool>`). Loopback-only and always
    // token-authenticated, like the proxy; each run gets a token naming its
    // project (which servers it may see) and its run (so a tool call is
    // attributable). Server secrets are resolved from the keep, scoped to the
    // server's project.
    let mcp_prefs = loaded.settings.mcp.clone();
    if mcp_prefs.enabled {
        let key = config::token::generate().unwrap_or_default();
        if !key.is_empty() {
            secrets::set_mcp_key(key.clone());
            match config::bind::guard(&mcp_prefs.bind, &key, config::bind::Policy::LoopbackOnly) {
                Ok(()) => {
                    tracing::info!(bind = %mcp_prefs.bind, "mcp gateway starting");
                    let bind = mcp_prefs.bind.clone();
                    let servers = loaded.settings.mcp_servers.clone();
                    let expose = mcp::Expose::parse(&mcp_prefs.expose);
                    let keep = keep_handle.clone();
                    let report_path = db_path.clone();
                    let audit_path = db_path.clone();
                    std::thread::spawn(move || {
                        // Spawn/connect each server, resolving its secrets from
                        // the keep scoped to its project (0 = global).
                        let (host, warnings) = mcp::connect(&servers, |project, name| {
                            let guard = keep.lock().unwrap_or_else(|e| e.into_inner());
                            guard.as_ref().and_then(|k| {
                                let scope = (project != 0).then_some(project);
                                k.resolve(scope, name).map(str::to_string)
                            })
                        });
                        for warning in &warnings {
                            report_server_problem(&report_path, "MCP server skipped", warning);
                        }
                        // Attribute every tool call to its run in the event log,
                        // so the Diff surface (and siblings) can see what an agent
                        // reached for.
                        let audit: mcp::AuditHook = Box::new(move |call: mcp::Audit| {
                            if call.run == 0 {
                                return;
                            }
                            if let Ok(db) = store::Db::open(&audit_path) {
                                let task = db.run(call.run).ok().map(|r| r.task_id);
                                let data = serde_json::json!({
                                    "tool": call.tool, "ok": call.ok, "project": call.project,
                                })
                                .to_string();
                                let _ = db.record_event(
                                    "mcp_call",
                                    task,
                                    Some(call.run),
                                    &data,
                                    unix_now(),
                                );
                            }
                        });
                        let gateway = mcp::Gateway {
                            key,
                            host,
                            expose,
                            audit: Some(audit),
                        };
                        if let Err(error) = mcp::serve(bind.as_str(), gateway) {
                            report_server_problem(
                                &report_path,
                                "MCP gateway stopped",
                                &format!("The MCP gateway failed: {error}"),
                            );
                        }
                    });
                }
                Err(refusal) => {
                    report_server_problem(&db_path, "MCP gateway disabled", &format!("{refusal}"))
                }
            }
        }
    }

    // Check GitHub Releases for a newer version and, once, drop an Inbox
    // notification pointing at the download. Non-blocking and opt-out.
    if std::env::var_os("ASYLUM_NO_UPDATE_CHECK").is_none() {
        std::thread::spawn(move || check_for_update(db_path));
    }

    gpui_platform::application()
        .with_assets(icons::Assets)
        .run(move |cx: &mut App| {
            theme::install(&loaded.settings, cx);

            let bounds = Bounds::centered(None, size(px(1200.0), px(820.0)), cx);
            let root = cx.new(|_cx| Root::seeded());
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        window_min_size: Some(size(px(720.0), px(480.0))),
                        // Transparent native titlebar: our own header draws the
                        // chrome and doubles as the window drag handle, with the
                        // macOS traffic lights floated into the header.
                        titlebar: Some(TitlebarOptions {
                            title: None,
                            appears_transparent: true,
                            traffic_light_position: Some(point(px(14.0), px(16.0))),
                        }),
                        ..Default::default()
                    },
                    {
                        let root = root.clone();
                        move |_window, _cx| root.clone()
                    },
                )
                .expect("open window");

            // Agents are real child process groups, and process teardown never
            // drops the terminal entities that would end them; kill whatever is
            // still live before the app goes away.
            let quit_root = root.downgrade();
            cx.on_app_quit(move |cx| {
                let pidfiles = quit_root
                    .update(cx, |root, _| root.take_run_pidfiles())
                    .unwrap_or_default();
                reap::terminate_all(pidfiles);
                async {}
            })
            .detach();

            // The full menu bar, keybindings, and their handlers.
            menus::install(root, window, &loaded.settings, cx);

            // Seed the root with the boot settings and live-reload on change.
            reload::init(window, loaded, cx);

            cx.activate(true);
        });
}

/// Unix seconds, for stamping audit events.
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Path to the encrypted secrets keep, alongside `settings.json`.
fn keep_path() -> std::path::PathBuf {
    config::default_path()
        .parent()
        .map(|dir| dir.join("keep.enc"))
        .unwrap_or_else(|| std::path::PathBuf::from("keep.enc"))
}

/// Post a server startup/runtime problem to the Inbox so a refused bind or a
/// failed listener is visible in the app instead of being silently discarded.
/// Best-effort: if the store cannot be opened there is nowhere to report.
fn report_server_problem(db_path: &std::path::Path, title: &str, body: &str) {
    tracing::warn!(%title, %body, "server problem");
    let Ok(db) = store::Db::open(db_path) else {
        return;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let _ = db.notify("server", title, body, None, now);
}

/// Check for a newer release and post a one-time Inbox notification. Runs on a
/// background thread; any failure (offline, no `curl`, no release yet) is
/// silently ignored so a launch never depends on the network.
fn check_for_update(db_path: std::path::PathBuf) {
    let repo = std::env::var("ASYLUM_UPDATE_REPO").unwrap_or_else(|_| "wess/asylum".to_string());
    let current = env!("CARGO_PKG_VERSION");
    let Ok(update::Status::Available(release)) = update::check(&repo, current) else {
        return;
    };
    let Ok(db) = store::Db::open(&db_path) else {
        return;
    };
    let title = format!("Update available: {}", release.tag);
    // Post at most once per version.
    let already = db
        .notifications(false)
        .unwrap_or_default()
        .into_iter()
        .any(|note| note.title == title);
    if already {
        return;
    }
    let body = notifications::update_body(&release.notes, &release.url);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let _ = db.notify("update", &title, &body, None, now);
}
