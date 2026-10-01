-- Action Recording (what Bots did: metadata only, kept 90 days) and the
-- audit log (control-plane changes: settings, plugins, policy, computer).
CREATE TABLE events (
  id      TEXT PRIMARY KEY,
  at      INTEGER NOT NULL,
  kind    TEXT NOT NULL,           -- 'action' or 'audit'
  actor   TEXT NOT NULL DEFAULT '',-- 'user', 'admin', or a Bot id
  bot_id  TEXT,
  chat_id TEXT,
  name    TEXT NOT NULL,           -- tool or event name
  target  TEXT NOT NULL DEFAULT '',
  outcome TEXT NOT NULL DEFAULT '',
  detail  TEXT NOT NULL DEFAULT ''
);
CREATE INDEX events_kind_at ON events (kind, at);

-- Value sizes, for Team Bot secret limits (values stay in the keychain).
ALTER TABLE secrets ADD COLUMN size INTEGER NOT NULL DEFAULT 0;
