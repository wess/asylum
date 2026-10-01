//! Process modes that run without the window:
//!
//! - `asylumdev ask <bot> <message>` sends a message (creating the Bot if
//!   needed), waits for it to finish, and prints the reply.
//! - `asylumdev providers` lists provider profiles and the default.
//! - `asylumdev plugin add <catalog-id>` adds a Marketplace plugin.
//! - `asylumdev pin <bot> <provider> [model]` puts a Bot on its own model
//!   (`asylumdev pin <bot> default` returns it to the default).

use crate::tk;
use agent::Event;

pub fn dispatch(args: &[String], rt: &agent::Runtime) -> Option<i32> {
  match args.first().map(String::as_str) {
    Some("ask") if args.len() >= 3 => Some(tk::runtime().block_on(ask(rt, &args[1], &args[2..].join(" ")))),
    Some("plugin") if args.get(1).map(String::as_str) == Some("add") && args.len() >= 3 => {
      let r = tk::runtime().block_on(agent::api::connect::add(rt, &args[2]));
      match r {
        Ok(p) => {
          println!("{} ({})", p.name, p.status);
          Some(0)
        }
        Err(e) => {
          eprintln!("{e}");
          Some(1)
        }
      }
    }
    Some("pin") if args.len() >= 3 => Some(tk::runtime().block_on(async {
      let Ok(Some(bot)) = store::bots::find(&rt.pool, &args[1]).await else {
        eprintln!("no Agent named {}", args[1]);
        return 1;
      };
      let (p, m) = if args[2] == "default" { (String::new(), String::new()) } else { (args[2].clone(), args.get(3).cloned().unwrap_or_default()) };
      if !p.is_empty() && rt.profile(&p).is_none() {
        eprintln!("no provider named {p} (see `asylumdev providers`)");
        return 1;
      }
      match store::bots::set_model(&rt.pool, &bot.id, &p, &m).await {
        Ok(()) => {
          println!("{} → {}", bot.name, if p.is_empty() { "default".to_string() } else { format!("{p} {m}") });
          0
        }
        Err(e) => {
          eprintln!("{e}");
          1
        }
      }
    })),
    Some("providers") => {
      let s = rt.settings();
      for p in rt.profiles() {
        let mark = if rt.profile("").is_some_and(|d| d.name == p.name) { "*" } else { " " };
        let at = if p.is_process() { p.command.clone() } else { p.endpoint.clone() };
        println!("{mark} {:<14} {:<12} {at}  ({})", p.name, p.preset, provider::credential::describe(&p.credential));
      }
      println!("default model: {}", if s.model.is_empty() { "(auto)" } else { &s.model });
      Some(0)
    }
    // A small real request: proves key, model name, and upstream access.
    Some("check") if args.len() >= 2 => Some(tk::runtime().block_on(async {
      let Some(p) = rt.profile(&args[1]) else {
        eprintln!("no provider named {} (see `asylumdev providers`)", args[1]);
        return 1;
      };
      let s = rt.settings();
      let model = args.get(2).cloned().filter(|m| !m.is_empty()).or_else(|| Some(p.model.clone()).filter(|m| !m.is_empty())).or_else(|| (s.provider == p.name).then(|| s.model.clone()).filter(|m| !m.is_empty())).or_else(|| p.models.first().cloned()).unwrap_or_default();
      if model.is_empty() {
        eprintln!("no model: pass one, e.g. `asylumdev check {} gpt-4o`", p.name);
        return 1;
      }
      match provider::check(&p, &model, &rt.computer.workspace()).await {
        Ok(reply) => {
          println!("{} {model}: ok ({reply})", p.name);
          0
        }
        Err(e) => {
          eprintln!("{} {model}: {e}", p.name);
          1
        }
      }
    })),
    // LiteLLM: the key's teams and the models each is scoped to.
    Some("teams") if args.len() >= 2 => Some(tk::runtime().block_on(async {
      let Some(p) = rt.profile(&args[1]) else {
        eprintln!("no provider named {}", args[1]);
        return 1;
      };
      let key = provider::credential::resolve_for(&p).await.ok().flatten().unwrap_or_default();
      match provider::litellm::teams(&p.endpoint, &key).await {
        Ok(teams) if teams.is_empty() => {
          println!("no teams: this key sees the gateway's whole catalog");
          0
        }
        Ok(teams) => {
          for t in teams {
            let mark = if p.team.as_deref() == Some(t.id.as_str()) { "*" } else { " " };
            let scope = if t.models.is_empty() { "all models".to_string() } else { t.models.join(", ") };
            println!("{mark} {:<24} {:<20} {scope}", t.name, t.id);
          }
          0
        }
        Err(e) => {
          eprintln!("{e}");
          1
        }
      }
    })),
    Some("demo") => Some(match tk::runtime().block_on(crate::demo::seed(rt)) {
      Ok(()) => {
        println!("demo data written to {}", config::data_dir().display());
        0
      }
      Err(e) => {
        eprintln!("{e}");
        1
      }
    }),
    _ => None,
  }
}

async fn ask(rt: &agent::Runtime, name: &str, text: &str) -> i32 {
  let bot = match store::bots::find(&rt.pool, name).await {
    Ok(Some(b)) => b,
    _ => match agent::api::bots::create(rt, Some(name)).await {
      Ok((b, _)) => b,
      Err(e) => {
        eprintln!("{e}");
        return 1;
      }
    },
  };
  let chat = match store::chats::direct(&rt.pool, &bot.id).await {
    Ok(c) => c,
    Err(e) => {
      eprintln!("{e}");
      return 1;
    }
  };
  let mut events = rt.subscribe();
  if let Err(e) = agent::api::chat::send(rt, &chat.id, text, &[], None).await {
    eprintln!("{e}");
    return 1;
  }
  let finished = tokio::time::timeout(std::time::Duration::from_secs(600), async {
    loop {
      match events.recv().await {
        Ok(Event::BotStatus { bot: b, status }) if b == bot.id && (status == "done" || status == "idle") => return status,
        Ok(Event::Approval { id }) => {
          if let Ok(a) = store::approvals::get(&rt.pool, &id).await {
            if a.status == "pending" {
              eprintln!("[approval needed] {} {} — approving once from the CLI", a.tool, a.target);
              let _ = agent::api::cards::approve(rt, &id, true, false).await;
            }
          }
        }
        Ok(_) => {}
        Err(_) => return "closed".to_string(),
      }
    }
  })
  .await;
  let msgs = store::messages::recent(&rt.pool, &chat.id, 3).await.unwrap_or_default();
  if let Some(m) = msgs.iter().rev().find(|m| m.role == "bot") {
    for p in agent::part::parse(&m.parts) {
      match p {
        agent::Part::Tool { name, result, status, .. } => eprintln!("[tool {name} {status}] {}", result.chars().take(200).collect::<String>()),
        agent::Part::Error { text } => eprintln!("[error] {text}"),
        _ => {}
      }
    }
    println!("{}", m.body);
  }
  match finished {
    Ok(_) => 0,
    Err(_) => {
      eprintln!("timed out");
      2
    }
  }
}
