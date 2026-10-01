//! One queue per Bot. A Bot works one job at a time; a new message from the
//! user preempts whatever the Bot is doing in that conversation, while
//! routine runs and handoffs wait their turn.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
  Chat,
  Routine,
  Test,
  Handoff,
}

impl Origin {
  pub fn as_str(self) -> &'static str {
    match self {
      Origin::Chat => store::runs::CHAT,
      Origin::Routine => store::runs::ROUTINE,
      Origin::Test => store::runs::TEST,
      Origin::Handoff => store::runs::HANDOFF,
    }
  }

  /// Work the user started waits on them indefinitely; anything else has
  /// its approvals expire.
  pub fn interactive(self) -> bool {
    self == Origin::Chat
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Job {
  pub bot: String,
  pub chat: String,
  pub origin: Origin,
  pub routine: Option<String>,
  /// Extra context for this job (a webhook body, a handoff note).
  pub note: String,
}

#[derive(Default)]
struct Lane {
  pending: VecDeque<Job>,
  running: Option<(Job, CancellationToken)>,
}

#[derive(Default)]
pub struct Queues {
  lanes: Mutex<HashMap<String, Lane>>,
}

pub enum Push {
  /// Nothing was running: start a worker.
  Start,
  /// A worker is running and will pick this up.
  Queued,
}

impl Queues {
  pub fn push(&self, job: Job) -> Push {
    let mut lanes = self.lanes.lock().expect("queue lock");
    let lane = lanes.entry(job.bot.clone()).or_default();
    if job.origin == Origin::Chat {
      // The newest user message supersedes queued replies to the same chat.
      lane.pending.retain(|j| !(j.origin == Origin::Chat && j.chat == job.chat));
      if let Some((running, token)) = &lane.running {
        if running.chat == job.chat {
          token.cancel();
        }
      }
      lane.pending.push_front(job);
    } else {
      lane.pending.push_back(job);
    }
    if lane.running.is_none() {
      Push::Start
    } else {
      Push::Queued
    }
  }

  /// Take the next job for a Bot's worker, marking it running.
  pub fn next(&self, bot: &str) -> Option<(Job, CancellationToken)> {
    let mut lanes = self.lanes.lock().expect("queue lock");
    let lane = lanes.get_mut(bot)?;
    match lane.pending.pop_front() {
      Some(job) => {
        let token = CancellationToken::new();
        lane.running = Some((job.clone(), token.clone()));
        Some((job, token))
      }
      None => {
        lane.running = None;
        None
      }
    }
  }

  /// Claim the worker slot for a Bot if nobody holds it.
  pub fn claim(&self, bot: &str) -> bool {
    let mut lanes = self.lanes.lock().expect("queue lock");
    let lane = lanes.entry(bot.to_string()).or_default();
    lane.running.is_none() && !lane.pending.is_empty()
  }

  /// Stop everything a Bot is doing and drop its queue ("Stop now").
  pub fn stop(&self, bot: &str) {
    let mut lanes = self.lanes.lock().expect("queue lock");
    if let Some(lane) = lanes.get_mut(bot) {
      lane.pending.clear();
      if let Some((_, token)) = &lane.running {
        token.cancel();
      }
    }
  }

  pub fn busy(&self, bot: &str) -> bool {
    let lanes = self.lanes.lock().expect("queue lock");
    lanes.get(bot).is_some_and(|l| l.running.is_some())
  }

  pub fn busy_any(&self) -> bool {
    let lanes = self.lanes.lock().expect("queue lock");
    lanes.values().any(|l| l.running.is_some())
  }
}

#[cfg(test)]
#[path = "../tests/queue.rs"]
mod tests;
