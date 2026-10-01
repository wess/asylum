//! Whether an action runs, needs the user, or is refused. Reads that cannot
//! change anything run freely; everything else goes through Auto-review when
//! it is on, and consequential actions ask the user when it is off.
//! Commands on the user's own machine follow the local-execution setting.

use crate::review::{self, Judgment};
use crate::runtime::{Reply, Runtime};
use anyhow::Result;
use config::LocalExec;
use std::time::Duration;
use store::approvals::{self, Approval};
use store::rules;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
  /// Reads, drafts, and conversation: never reviewed.
  Free,
  /// Changes on the sandboxed computer: reviewed when Auto-review is on.
  Review,
  /// External or irreversible (plugin writes, deletions, delegation):
  /// reviewed when Auto-review is on, otherwise the user approves.
  Consequential,
  /// Commands and file access on the user's own machine.
  Local,
}

pub enum Verdict {
  Run,
  Deny(String),
  Ask(approvals::New),
}

pub enum Decision {
  Allowed,
  Denied,
  Expired,
  Stopped,
}

/// Plugin tools whose names read as writes are consequential unless the
/// server marks them read-only.
pub fn plugin_class(tool: &str, read_only: bool) -> Class {
  if read_only {
    return Class::Free;
  }
  let t = tool.to_lowercase();
  const READS: [&str; 12] = ["get", "list", "search", "read", "fetch", "find", "query", "view", "lookup", "describe", "ask", "question"];
  if READS.iter().any(|r| t.starts_with(r) || t.contains(&format!("_{r}"))) && !t.contains("send") {
    Class::Free
  } else {
    Class::Consequential
  }
}

/// The rule saved by "Always allow".
pub fn signature(tool: &str, target: &str) -> String {
  if target.is_empty() {
    format!("Always allow `{tool}`")
  } else {
    format!("Always allow `{tool}` on `{target}`")
  }
}

pub struct Action<'a> {
  pub bot: &'a str,
  pub chat: &'a str,
  pub run: &'a str,
  pub interactive: bool,
  pub tool: &'a str,
  pub target: &'a str,
  pub args: &'a str,
  pub class: Class,
  /// The user's request this action serves, for Auto-review.
  pub request: &'a str,
  pub profile: &'a str,
}

pub async fn check(rt: &Runtime, a: &Action<'_>) -> Result<Verdict> {
  let settings = rt.settings();
  let ask = |kind: &str, reason: String| approvals::New {
    bot: a.bot.to_string(),
    chat: Some(a.chat.to_string()),
    run: Some(a.run.to_string()),
    kind: kind.to_string(),
    tool: a.tool.to_string(),
    target: a.target.to_string(),
    args: a.args.to_string(),
    reason,
    interactive: a.interactive,
  };
  if a.class == Class::Free {
    return Ok(Verdict::Run);
  }
  if a.class == Class::Local {
    return Ok(match rt.policy().cap_local(settings.local_exec) {
      LocalExec::Never => Verdict::Deny("Execution on Local Computer is set to Never allow.".into()),
      LocalExec::Always => Verdict::Run,
      LocalExec::Ask => {
        if allowed(rt, a).await? {
          Verdict::Run
        } else {
          Verdict::Ask(ask(approvals::LOCAL, "Run on your local computer".into()))
        }
      }
    });
  }
  if allowed(rt, a).await? {
    return Ok(Verdict::Run);
  }
  if rt.policy().auto_review(settings.auto_review) {
    let rules = rules::all(&rt.pool).await?;
    return Ok(match review::judge(rt, a, &rules).await {
      Judgment::Proceed => Verdict::Run,
      Judgment::Deny(why) => Verdict::Deny(why),
      Judgment::Ask(why) => Verdict::Ask(ask(approvals::REVIEW, why)),
    });
  }
  Ok(match a.class {
    Class::Consequential => Verdict::Ask(ask(approvals::APPROVAL, String::new())),
    _ => Verdict::Run,
  })
}

async fn allowed(rt: &Runtime, a: &Action<'_>) -> Result<bool> {
  let exact = signature(a.tool, a.target);
  let broad = signature(a.tool, "");
  Ok(
    rules::all(&rt.pool)
      .await?
      .iter()
      .any(|r| r.kind == rules::ALLOW && (r.text == exact || r.text == broad)),
  )
}

/// Wait for the user to answer an approval card. Background work expires
/// after the window; "Stop now" or preemption cancels.
pub async fn wait(rt: &Runtime, approval: &Approval, cancel: &CancellationToken) -> Result<Decision> {
  let rx = rt.wait(&approval.id).await;
  let expiry = async {
    if approval.interactive {
      std::future::pending::<()>().await
    } else {
      tokio::time::sleep(Duration::from_millis(approvals::EXPIRY_MS as u64)).await
    }
  };
  let decision = tokio::select! {
    r = rx => match r {
      Ok(Reply::Approve { always }) => {
        approvals::decide(&rt.pool, &approval.id, approvals::APPROVED).await?;
        if always {
          rules::add(&rt.pool, rules::ALLOW, &signature(&approval.tool, &approval.target), false).await?;
        }
        Decision::Allowed
      }
      Ok(Reply::Deny) => {
        approvals::decide(&rt.pool, &approval.id, approvals::DENIED).await?;
        Decision::Denied
      }
      _ => {
        approvals::decide(&rt.pool, &approval.id, approvals::CANCELED).await?;
        Decision::Stopped
      }
    },
    _ = cancel.cancelled() => {
      approvals::decide(&rt.pool, &approval.id, approvals::CANCELED).await?;
      Decision::Stopped
    }
    _ = expiry => {
      approvals::decide(&rt.pool, &approval.id, approvals::EXPIRED).await?;
      Decision::Expired
    }
  };
  rt.emit(crate::Event::Approval { id: approval.id.clone() });
  Ok(decision)
}

#[cfg(test)]
#[path = "../tests/approve.rs"]
mod tests;
