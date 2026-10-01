//! Asylum: always-on AI teammates with their own computer.

mod account;
mod actions;
mod audio;
mod avatar;
mod cards;
mod chat;
mod cli;
mod composer;
mod computer;
mod details;
mod i18n;
mod market;
mod menu;
mod menus;
mod newchat;
mod notify;
mod onboarding;
mod palette;
mod root;
mod settings;
mod sidebar;
mod state;
mod template;
mod theme;
mod tk;
mod voicechat;

use futures::StreamExt;
use gpui::App;

fn main() {
  let load = config::load(&config::settings_path());
  i18n::set(&load.settings.language);
  for d in &load.diagnostics {
    eprintln!("settings: {d}");
  }
  let data = config::data_dir();
  let _enter = tk::runtime().enter();
  let rt = match tk::runtime().block_on(agent::Runtime::start(data, load.settings.clone())) {
    Ok(rt) => rt,
    Err(e) => {
      eprintln!("could not start: {e:#}");
      std::process::exit(1);
    }
  };
  let args: Vec<String> = std::env::args().skip(1).collect();
  if let Some(code) = cli::dispatch(&args, &rt) {
    std::process::exit(code);
  }
  let _ = tk::runtime().block_on(agent::policy::apply(&rt, &config::policy::path()));
  agent::ticker::spawn(rt.clone());
  {
    let rt = rt.clone();
    tk::spawn(async move {
      let _ = rt.refresh_models().await;
    });
  }
  {
    let rt = rt.clone();
    tk::spawn(async move {
      let _ = agent::webhook::serve(rt).await;
    });
  }
  {
    let rt = rt.clone();
    tk::spawn(async move {
      let _ = agent::admin::serve(rt).await;
    });
  }
  {
    // Killed (SIGTERM/SIGINT): close the browser first so no headless
    // Chromium is left holding the profile.
    let rt = rt.clone();
    tk::spawn(async move {
      use tokio::signal::unix::{signal, SignalKind};
      let (Ok(mut term), Ok(mut int)) = (signal(SignalKind::terminate()), signal(SignalKind::interrupt())) else { return };
      tokio::select! {
        _ = term.recv() => {}
        _ = int.recv() => {}
      }
      rt.browser.shutdown().await;
      std::process::exit(0);
    });
  }
  {
    let rt = rt.clone();
    tk::spawn(async move {
      let _ = agent::api::computer::start(&rt).await;
      agent::api::slack::start_all(&rt).await;
    });
  }

  let app = gpui_platform::application();
  let rt2 = rt.clone();
  app.on_reopen(move |cx| {
    if cx.windows().is_empty() {
      let _ = open(rt2.clone(), cx);
    }
  });
  app.run(move |cx: &mut App| {
    actions::bind(cx);
    menus::set(cx);
    cx.on_action(|_: &actions::Quit, cx| cx.quit());
    // Stop the computer's browser with the app so it never outlives us.
    let quit_rt = rt.clone();
    cx.on_app_quit(move |_| {
      let rt = quit_rt.clone();
      async move {
        let _ = tk::spawn(async move { rt.browser.shutdown().await }).await;
      }
    })
    .detach();
    let _ = open(rt.clone(), cx);
    watch(rt.clone(), cx);
    cx.activate(true);
  });
}

fn open(rt: agent::Runtime, cx: &mut App) -> anyhow::Result<()> {
  let handle = root::open(rt.clone(), cx)?;
  // Forward engine events into this window.
  let (tx, mut rx) = futures::channel::mpsc::unbounded();
  let mut events = rt.subscribe();
  tk::spawn(async move {
    loop {
      match events.recv().await {
        Ok(e) => {
          if tx.unbounded_send(e).is_err() {
            break;
          }
        }
        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
        Err(_) => break,
      }
    }
  });
  cx.spawn(async move |cx| {
    while let Some(e) = rx.next().await {
      if handle.update(cx, |root, window, cx| root.on_event(e, window, cx)).is_err() {
        break;
      }
    }
  })
  .detach();
  Ok(())
}

/// Live-reload bots.json.
fn watch(rt: agent::Runtime, cx: &mut App) {
  let (tx, mut rx) = futures::channel::mpsc::unbounded::<()>();
  let handle = config::watch(&config::settings_path(), move || {
    let _ = tx.unbounded_send(());
  });
  cx.spawn(async move |cx| {
    let _keep = handle;
    while rx.next().await.is_some() {
      let load = config::load(&config::settings_path());
      i18n::set(&load.settings.language);
      rt.set_settings(load.settings);
      cx.update(|cx| cx.refresh_windows());
    }
  })
  .detach();
}
