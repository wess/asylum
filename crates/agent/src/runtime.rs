use crate::event::Event;
use crate::plugins::Hub;
use crate::queue::Queues;
use anyhow::{anyhow, Result};
use computer::browser::Browser;
use computer::Computer;
use config::Settings;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use store::Pool;
use tokio::sync::{broadcast, oneshot, Mutex, Semaphore};

/// A user's answer to something a run is waiting on.
#[derive(Clone, Debug, PartialEq)]
pub enum Reply {
  /// An approval decision: allow, and whether to always allow.
  Approve { always: bool },
  Deny,
  /// Takeover finished (`true`) or skipped.
  Takeover(bool),
  /// A secret saved (and optionally filled into the page).
  Secret { filled: Option<bool> },
  Cancel,
}

pub struct Inner {
  pub pool: Pool,
  pub data: PathBuf,
  pub computer: Computer,
  pub browser: Browser,
  pub settings: RwLock<Settings>,
  pub policy: RwLock<config::Policy>,
  /// The rule the egress proxy enforces (the policy's Network Controls).
  pub network: Arc<RwLock<config::policy::Network>>,
  /// The egress proxy's port.
  pub proxy: u16,
  /// Why the policy file couldn't be read, if it couldn't.
  pub policy_error: RwLock<Option<String>>,
  pub events: broadcast::Sender<Event>,
  pub queues: Queues,
  pub waiters: Mutex<HashMap<String, oneshot::Sender<Reply>>>,
  pub plugins: Hub,
  pub slots: Semaphore,
  /// Jobs hold this for reading; recreating the computer takes it for
  /// writing, so Bots pause between jobs and resume on the new computer.
  pub gate: tokio::sync::RwLock<()>,
  pub models: RwLock<Vec<String>>,
  pub recordings: Mutex<HashMap<String, crate::teach::Recording>>,
  /// Running Slack connections, by Team Bot.
  pub slack: Mutex<HashMap<String, tokio::task::JoinHandle<()>>>,
}

#[derive(Clone)]
pub struct Runtime(pub Arc<Inner>);

impl std::ops::Deref for Runtime {
  type Target = Inner;
  fn deref(&self) -> &Inner {
    &self.0
  }
}

impl Runtime {
  /// Open the database, prepare the computer, and settle anything a
  /// previous session left mid-flight.
  pub async fn start(data: PathBuf, settings: Settings) -> Result<Self> {
    let pool = store::open(&data.join("asylum.db")).await?;
    store::messages::settle_streaming(&pool).await?;
    store::runs::settle(&pool).await?;
    store::approvals::cancel_all(&pool).await?;
    sqlx_reset_status(&pool).await?;
    let computer = Computer::new(&data);
    computer.ensure()?;
    let browser = Browser::new(computer.profile());
    let (events, _) = broadcast::channel(1024);
    let slots = Semaphore::new(settings.max_parallel_bots.max(1) as usize);
    let network: Arc<RwLock<config::policy::Network>> = Arc::default();
    let rule_net = network.clone();
    let proxy = computer::proxy::start(Arc::new(move |host: &str| rule_net.read().map(|n| n.allows(host)).unwrap_or(false))).await?;
    let rt = Runtime(Arc::new(Inner {
      pool,
      data,
      computer,
      browser,
      settings: RwLock::new(settings),
      policy: RwLock::new(config::Policy::default()),
      network,
      proxy,
      policy_error: RwLock::new(None),
      events,
      queues: Queues::default(),
      waiters: Mutex::new(HashMap::new()),
      plugins: Hub::default(),
      slots,
      gate: tokio::sync::RwLock::new(()),
      models: RwLock::new(Vec::new()),
      recordings: Mutex::new(HashMap::new()),
      slack: Mutex::new(HashMap::new()),
    }));
    Ok(rt)
  }

  pub fn subscribe(&self) -> broadcast::Receiver<Event> {
    self.events.subscribe()
  }

  pub fn emit(&self, e: Event) {
    let _ = self.events.send(e);
  }

  pub fn settings(&self) -> Settings {
    self.settings.read().map(|s| s.clone()).unwrap_or_default()
  }

  /// Outbound network for computer commands under Network Controls.
  pub fn net(&self) -> computer::sandbox::Net {
    match self.policy().network.mode.as_str() {
      "offline" => computer::sandbox::Net::Offline,
      "" | "open" => computer::sandbox::Net::Open,
      _ => computer::sandbox::Net::Proxy(self.proxy),
    }
  }

  /// The egress proxy for web tools, when Network Controls apply.
  pub fn web_proxy(&self) -> Option<u16> {
    match self.net() {
      computer::sandbox::Net::Open => None,
      _ => Some(self.proxy),
    }
  }

  /// Outbound network for commands on the user's own Mac.
  pub fn local_net(&self) -> computer::sandbox::Net {
    if self.policy().local_egress_allowed() {
      computer::sandbox::Net::Open
    } else {
      computer::sandbox::Net::Offline
    }
  }

  pub fn policy(&self) -> config::Policy {
    self.policy.read().map(|p| p.clone()).unwrap_or_default()
  }

  pub fn set_settings(&self, s: Settings) {
    if let Ok(mut w) = self.settings.write() {
      *w = s;
    }
  }

  /// The xAI API, for what only xAI offers here: voice and images.
  pub fn xai(&self) -> Result<grok::Client> {
    let key = config::secret::xai_key().ok_or_else(|| anyhow!("Add your xAI API key in Settings to use voice and images."))?;
    Ok(grok::Client::new(key, None))
  }

  /// Every provider profile, with the xAI preset if none are configured.
  pub fn profiles(&self) -> Vec<config::Profile> {
    let s = self.settings();
    if s.providers.is_empty() {
      vec![provider::preset::default_profile()]
    } else {
      s.providers
    }
  }

  pub fn profile(&self, name: &str) -> Option<config::Profile> {
    let all = self.profiles();
    if name.is_empty() {
      let s = self.settings();
      return all.iter().find(|p| p.name == s.provider).cloned().or_else(|| all.first().cloned());
    }
    all.into_iter().find(|p| p.name == name)
  }

  /// The model a profile should use: the wanted one, else what the profile
  /// knows, else (for xAI) the best Grok model the key can use.
  fn pick_model(&self, profile: &config::Profile, wanted: &str, fast: bool) -> String {
    if !wanted.is_empty() {
      return wanted.to_string();
    }
    if profile.preset == "xai" {
      let avail = self.models.read().map(|m| m.clone()).unwrap_or_default();
      let s = self.settings();
      return if fast {
        crate::models::pick(&avail, &s.fast_model, &crate::models::FAST)
      } else {
        crate::models::pick(&avail, &s.model, &crate::models::MAIN)
      };
    }
    profile.models.first().cloned().unwrap_or_default()
  }

  /// The provider a Bot talks through: its own pin, else the default.
  pub async fn provider(&self, bot: Option<&store::Bot>) -> Result<provider::Provider> {
    let s = self.settings();
    let (pname, model) = match bot {
      Some(b) if !b.provider.is_empty() => (b.provider.clone(), b.model.clone()),
      _ => (s.provider.clone(), if s.provider.is_empty() || self.profile(&s.provider).is_some_and(|p| p.preset == "xai") { String::new() } else { s.model.clone() }),
    };
    let profile = self.profile(&pname).ok_or_else(|| anyhow!("The provider \"{pname}\" is gone. Pick another in Settings → Providers."))?;
    let model = self.pick_model(&profile, &model, false);
    if model.is_empty() {
      anyhow::bail!("Pick a model for {} in Settings → Providers.", profile.name);
    }
    if profile.preset == "xai" && config::secret::xai_key().is_none() {
      anyhow::bail!("Add your xAI API key in Settings → Providers to wake your Bots.");
    }
    if profile.kind == config::Kind::Process && !self.policy().cloud_agents_allowed() {
      anyhow::bail!("Your admin has turned off Cloud Agents ({}). Pick another provider for this Bot.", profile.name);
    }
    provider::Provider::open(&profile, &model, &self.computer.workspace(), s.retries).await
  }

  /// The provider for quick background work (Auto-review, memory, names).
  pub async fn fast(&self) -> Result<provider::Provider> {
    let s = self.settings();
    if s.fast_provider.is_empty() {
      let profile = self.profile("").ok_or_else(|| anyhow!("no provider"))?;
      let model = if profile.preset == "xai" { self.pick_model(&profile, "", true) } else { self.pick_model(&profile, &s.model, false) };
      return provider::Provider::open(&profile, &model, &self.computer.workspace(), s.retries).await;
    }
    let profile = self.profile(&s.fast_provider).ok_or_else(|| anyhow!("the fast provider is gone"))?;
    let model = self.pick_model(&profile, &s.fast_model, true);
    provider::Provider::open(&profile, &model, &self.computer.workspace(), s.retries).await
  }

  /// Learn which Grok models the xAI key can use.
  pub async fn refresh_models(&self) -> Result<()> {
    let list = self.xai()?.models().await?;
    if let Ok(mut m) = self.models.write() {
      *m = list.into_iter().map(|m| m.id).collect();
    }
    Ok(())
  }

  pub fn tz(&self) -> chrono_tz::Tz {
    schedule::zone(&self.settings().timezone)
  }

  /// Hand a waiting run the user's answer. Returns false if nothing waits.
  pub async fn reply(&self, key: &str, reply: Reply) -> bool {
    match self.waiters.lock().await.remove(key) {
      Some(tx) => tx.send(reply).is_ok(),
      None => false,
    }
  }

  pub async fn wait(&self, key: &str) -> oneshot::Receiver<Reply> {
    let (tx, rx) = oneshot::channel();
    self.waiters.lock().await.insert(key.to_string(), tx);
    rx
  }

  pub async fn is_waiting(&self, key: &str) -> bool {
    self.waiters.lock().await.contains_key(key)
  }
}

async fn sqlx_reset_status(pool: &Pool) -> Result<()> {
  for b in store::bots::list(pool).await? {
    if b.status != store::bots::IDLE {
      store::bots::set_status(pool, &b.id, store::bots::IDLE).await?;
    }
  }
  Ok(())
}
