//! Provider accounts and their usage snapshots.

use store::{Account, Usage};

use crate::state::{now, Root};

/// An account with its latest usage snapshot, for the Accounts view.
#[derive(Clone)]
pub struct AccountRow {
    pub account: Account,
    pub usage: Option<Usage>,
}

impl Root {
    /// Add a provider account from the `provider: label` input. The first run
    /// against an agent verifies the credential; usage fills in when a provider
    /// reports it. The newly added account becomes active.
    pub fn add_account_from_input(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(input) = self.account_input.clone() else {
            return;
        };
        let raw = input.read(cx).text();
        let (provider, label) = match raw.split_once(':') {
            Some((provider, label)) => (provider.trim(), label.trim()),
            None => (raw.trim(), "default"),
        };
        if provider.is_empty() {
            self.push_error("Account needs a provider", "Write it as provider: label.");
            return;
        }
        let label = if label.is_empty() { "default" } else { label };
        match self.db.add_account(provider, label, now()) {
            Ok(account) => {
                let _ = self.db.activate_account(account.id);
                input.update(cx, |input, cx| input.set_text("", cx));
                cx.notify();
            }
            Err(error) => self.push_error("Could not add account", error.to_string()),
        }
    }

    /// Accounts with their latest usage, grouped for the Accounts view.
    pub fn accounts(&self) -> Vec<AccountRow> {
        self.db
            .accounts(None)
            .unwrap_or_default()
            .into_iter()
            .map(|account| AccountRow {
                usage: self.db.latest_usage(account.id).ok().flatten(),
                account,
            })
            .collect()
    }
}
