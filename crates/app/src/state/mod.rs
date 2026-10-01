//! What the sidebar and chrome show, loaded from the store in one go. The
//! UI reloads it whenever the engine says Bots or chats changed.

use agent::Runtime;
use anyhow::Result;
use std::collections::HashMap;
use store::{Approval, Bot, Chat, Section};

#[derive(Clone, Default)]
pub struct Snap {
  pub bots: Vec<Bot>,
  pub sections: Vec<Section>,
  pub groups: Vec<Chat>,
  /// Each Bot's current conversation.
  pub direct: HashMap<String, Chat>,
  pub approvals: Vec<Approval>,
  pub usage: agent::usage::Summary,
}

/// A sidebar entry.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Item {
  Bot(String),
  Group(String),
}

impl Snap {
  pub fn bot(&self, id: &str) -> Option<&Bot> {
    self.bots.iter().find(|b| b.id == id)
  }

  pub fn chat_for(&self, item: &Item) -> Option<&Chat> {
    match item {
      Item::Bot(b) => self.direct.get(b),
      Item::Group(g) => self.groups.iter().find(|c| &c.id == g),
    }
  }

  /// The item that owns a chat.
  pub fn item_of(&self, chat: &str) -> Option<Item> {
    if let Some(g) = self.groups.iter().find(|c| c.id == chat) {
      return Some(Item::Group(g.id.clone()));
    }
    self.direct.iter().find(|(_, c)| c.id == chat).map(|(b, _)| Item::Bot(b.clone()))
  }

  /// Visible sidebar items in order: pinned first, then by section order,
  /// then unassigned. Hidden Bots are excluded.
  pub fn visible(&self) -> Vec<Item> {
    let mut pinned = Vec::new();
    let mut rest = Vec::new();
    for b in &self.bots {
      if b.hidden {
        continue;
      }
      let it = (Item::Bot(b.id.clone()), b.section_id.clone(), b.active);
      if b.pinned {
        pinned.push(it);
      } else {
        rest.push(it);
      }
    }
    for g in &self.groups {
      if g.hidden {
        continue;
      }
      let it = (Item::Group(g.id.clone()), g.section_id.clone(), g.updated);
      if g.pinned {
        pinned.push(it);
      } else {
        rest.push(it);
      }
    }
    let order = |s: &Option<String>| match s {
      Some(id) => self.sections.iter().position(|x| &x.id == id).unwrap_or(usize::MAX - 1),
      None => usize::MAX,
    };
    rest.sort_by_key(|(_, s, _)| order(s));
    pinned.into_iter().chain(rest).map(|(i, _, _)| i).collect()
  }

  pub fn hidden(&self) -> Vec<&Bot> {
    self.bots.iter().filter(|b| b.hidden).collect()
  }

  pub fn unread(&self) -> usize {
    self.direct.values().chain(self.groups.iter()).filter(|c| c.unread || c.attention).count()
  }

  pub fn section_of(&self, item: &Item) -> Option<String> {
    match item {
      Item::Bot(b) => self.bot(b).and_then(|b| b.section_id.clone()),
      Item::Group(g) => self.groups.iter().find(|c| &c.id == g).and_then(|c| c.section_id.clone()),
    }
  }

  pub fn pinned(&self, item: &Item) -> bool {
    match item {
      Item::Bot(b) => self.bot(b).is_some_and(|b| b.pinned),
      Item::Group(g) => self.groups.iter().any(|c| &c.id == g && c.pinned),
    }
  }
}

pub async fn load(rt: Runtime) -> Result<Snap> {
  let pool = &rt.pool;
  let bots = store::bots::list(pool).await?;
  let mut direct = HashMap::new();
  for b in &bots {
    direct.insert(b.id.clone(), store::chats::direct(pool, &b.id).await?);
  }
  Ok(Snap {
    sections: store::sections::all(pool).await?,
    groups: store::chats::groups(pool).await?,
    approvals: store::approvals::pending(pool).await?,
    usage: agent::usage::summary(&rt).await.unwrap_or_default(),
    direct,
    bots,
  })
}
