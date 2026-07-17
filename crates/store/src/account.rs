//! Provider accounts + usage snapshots.
//!
//! An account belongs to a provider (`claude`, `codex`, …). Exactly one account
//! per provider is `active` - [`Db::activate_account`] hot-swaps it. Usage
//! snapshots record consumption against a limit with a reset time, so the UI can
//! show "Claude: 62% · resets in 3h".

use rusqlite::{params, Row};

use crate::model::{Account, Usage};
use crate::{Db, Error, Result};

fn account_row(row: &Row) -> rusqlite::Result<Account> {
    Ok(Account {
        id: row.get("id")?,
        provider: row.get("provider")?,
        label: row.get("label")?,
        active: row.get::<_, i64>("active")? != 0,
        created_at: row.get("created_at")?,
    })
}

fn usage_row(row: &Row) -> rusqlite::Result<Usage> {
    Ok(Usage {
        id: row.get("id")?,
        account_id: row.get("account_id")?,
        used: row.get("used")?,
        limit: row.get("limit_")?,
        resets_at: row.get("resets_at")?,
        captured_at: row.get("captured_at")?,
    })
}

impl Db {
    /// Add an account. The first account for a provider becomes active.
    pub fn add_account(&self, provider: &str, label: &str, now: i64) -> Result<Account> {
        let existing: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM accounts WHERE provider = ?1",
            params![provider],
            |r| r.get(0),
        )?;
        let active = (existing == 0) as i64;
        self.conn().execute(
            "INSERT INTO accounts (provider, label, active, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![provider, label, active, now],
        )?;
        self.account(self.conn().last_insert_rowid())
    }

    /// Fetch one account.
    pub fn account(&self, id: i64) -> Result<Account> {
        self.conn()
            .query_row(
                "SELECT * FROM accounts WHERE id = ?1",
                params![id],
                account_row,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
                other => other.into(),
            })
    }

    /// All accounts for a provider (or all, when `provider` is `None`).
    pub fn accounts(&self, provider: Option<&str>) -> Result<Vec<Account>> {
        let conn = self.conn();
        match provider {
            Some(p) => {
                let mut stmt =
                    conn.prepare("SELECT * FROM accounts WHERE provider = ?1 ORDER BY created_at")?;
                let rows = stmt.query_map(params![p], account_row)?;
                Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
            }
            None => {
                let mut stmt =
                    conn.prepare("SELECT * FROM accounts ORDER BY provider, created_at")?;
                let rows = stmt.query_map([], account_row)?;
                Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
            }
        }
    }

    /// The active account for a provider, if one exists.
    pub fn active_account(&self, provider: &str) -> Result<Option<Account>> {
        match self.conn().query_row(
            "SELECT * FROM accounts WHERE provider = ?1 AND active = 1",
            params![provider],
            account_row,
        ) {
            Ok(a) => Ok(Some(a)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Hot-swap the active account for its provider to `id`.
    pub fn activate_account(&self, id: i64) -> Result<()> {
        let account = self.account(id)?;
        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            "UPDATE accounts SET active = 0 WHERE provider = ?1",
            params![account.provider],
        )?;
        tx.execute("UPDATE accounts SET active = 1 WHERE id = ?1", params![id])?;
        tx.commit()?;
        Ok(())
    }

    /// Delete an account (and its usage rows via cascade). If it was active,
    /// promote the oldest remaining account for the same provider.
    pub fn delete_account(&self, id: i64) -> Result<()> {
        let account = self.account(id)?;
        let tx = self.conn().unchecked_transaction()?;
        tx.execute("DELETE FROM accounts WHERE id = ?1", params![id])?;
        if account.active {
            tx.execute(
                "UPDATE accounts SET active = 1 WHERE id = (
                    SELECT id FROM accounts WHERE provider = ?1
                    ORDER BY created_at, id LIMIT 1
                )",
                params![account.provider],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Record a usage snapshot for an account.
    pub fn record_usage(
        &self,
        account_id: i64,
        used: i64,
        limit: Option<i64>,
        resets_at: Option<i64>,
        now: i64,
    ) -> Result<Usage> {
        self.conn().execute(
            "INSERT INTO usage (account_id, used, limit_, resets_at, captured_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![account_id, used, limit, resets_at, now],
        )?;
        let id = self.conn().last_insert_rowid();
        self.conn()
            .query_row("SELECT * FROM usage WHERE id = ?1", params![id], usage_row)
            .map_err(Into::into)
    }

    /// The latest usage snapshot for an account, if any.
    pub fn latest_usage(&self, account_id: i64) -> Result<Option<Usage>> {
        match self.conn().query_row(
            "SELECT * FROM usage WHERE account_id = ?1 ORDER BY captured_at DESC, id DESC LIMIT 1",
            params![account_id],
            usage_row,
        ) {
            Ok(u) => Ok(Some(u)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}
