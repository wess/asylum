//! The notification inbox: store + desktop delivery and unread badges.

use store::Notification;

use crate::state::{now, Root};

impl Root {
    /// The notification inbox, newest first.
    pub fn notifications(&self) -> Vec<Notification> {
        self.db.notifications(false).unwrap_or_default()
    }

    /// Unread notification count (badge).
    pub fn unread(&self) -> usize {
        self.db.unread_count().unwrap_or(0)
    }

    /// Store a notification and post a desktop notification for it.
    pub(crate) fn push_notification(
        &self,
        kind: &str,
        title: &str,
        body: &str,
        run_id: Option<i64>,
    ) {
        let _ = self.db.notify(kind, title, body, run_id, now());
        let _ = notify::send(&notify::Notification::new(title, body));
    }
}
