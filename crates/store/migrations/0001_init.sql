CREATE TABLE teams (
  id      TEXT PRIMARY KEY,
  name    TEXT NOT NULL,
  created INTEGER NOT NULL
);

CREATE TABLE team_members (
  team_id TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
  email   TEXT NOT NULL,
  name    TEXT NOT NULL DEFAULT '',
  role    TEXT NOT NULL DEFAULT 'member',
  created INTEGER NOT NULL,
  PRIMARY KEY (team_id, email)
);

CREATE TABLE sections (
  id       TEXT PRIMARY KEY,
  name     TEXT NOT NULL,
  position INTEGER NOT NULL DEFAULT 0,
  created  INTEGER NOT NULL
);

-- kind: personal | team | helper | system
CREATE TABLE bots (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  label         TEXT NOT NULL DEFAULT '',
  description   TEXT NOT NULL DEFAULT '',
  avatar        TEXT NOT NULL DEFAULT '',
  color         TEXT NOT NULL DEFAULT '',
  kind          TEXT NOT NULL DEFAULT 'personal',
  notifications INTEGER NOT NULL DEFAULT 1,
  pinned        INTEGER NOT NULL DEFAULT 0,
  hidden        INTEGER NOT NULL DEFAULT 0,
  primary_bot   INTEGER NOT NULL DEFAULT 0,
  position      INTEGER NOT NULL DEFAULT 0,
  section_id    TEXT REFERENCES sections(id) ON DELETE SET NULL,
  team_id       TEXT REFERENCES teams(id) ON DELETE SET NULL,
  owner         TEXT NOT NULL DEFAULT '',
  creator_id    TEXT REFERENCES bots(id) ON DELETE SET NULL,
  published     INTEGER NOT NULL DEFAULT 0,
  required      INTEGER NOT NULL DEFAULT 0,
  status        TEXT NOT NULL DEFAULT 'idle',
  created       INTEGER NOT NULL,
  updated       INTEGER NOT NULL,
  active        INTEGER NOT NULL DEFAULT 0
);

-- A Bot's conversation(s) and group chats. `unread` / `attention` drive the
-- sidebar badges; `draft` keeps an unsent composer per conversation.
CREATE TABLE chats (
  id          TEXT PRIMARY KEY,
  kind        TEXT NOT NULL,
  title       TEXT NOT NULL DEFAULT '',
  description TEXT NOT NULL DEFAULT '',
  bot_id      TEXT REFERENCES bots(id) ON DELETE CASCADE,
  section_id  TEXT REFERENCES sections(id) ON DELETE SET NULL,
  pinned      INTEGER NOT NULL DEFAULT 0,
  hidden      INTEGER NOT NULL DEFAULT 0,
  unread      INTEGER NOT NULL DEFAULT 0,
  attention   INTEGER NOT NULL DEFAULT 0,
  draft       TEXT NOT NULL DEFAULT '',
  position    INTEGER NOT NULL DEFAULT 0,
  created     INTEGER NOT NULL,
  updated     INTEGER NOT NULL
);
CREATE INDEX chats_bot ON chats(bot_id);

CREATE TABLE chat_members (
  chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
  bot_id  TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
  PRIMARY KEY (chat_id, bot_id)
);

-- role: user | bot | event. `parts` is JSON (attachments, tool calls, cards).
-- `thread_id` points at the message a reply belongs to.
CREATE TABLE messages (
  id        TEXT PRIMARY KEY,
  chat_id   TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
  bot_id    TEXT REFERENCES bots(id) ON DELETE SET NULL,
  role      TEXT NOT NULL,
  body      TEXT NOT NULL DEFAULT '',
  parts     TEXT NOT NULL DEFAULT '[]',
  status    TEXT NOT NULL DEFAULT 'done',
  run_id    TEXT,
  thread_id TEXT,
  reactions TEXT NOT NULL DEFAULT '{}',
  created   INTEGER NOT NULL
);
CREATE INDEX messages_chat ON messages(chat_id, created);
CREATE INDEX messages_thread ON messages(thread_id);
CREATE VIRTUAL TABLE messages_fts USING fts5(body, content='messages', content_rowid='rowid');
CREATE TRIGGER messages_ai AFTER INSERT ON messages BEGIN
  INSERT INTO messages_fts(rowid, body) VALUES (new.rowid, new.body);
END;
CREATE TRIGGER messages_ad AFTER DELETE ON messages BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, body) VALUES ('delete', old.rowid, old.body);
END;
CREATE TRIGGER messages_au AFTER UPDATE OF body ON messages BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, body) VALUES ('delete', old.rowid, old.body);
  INSERT INTO messages_fts(rowid, body) VALUES (new.rowid, new.body);
END;

-- scope: bot (personal memory) | team (Team Bot shared memory) | person
-- (a Team Bot's private notes with one teammate).
CREATE TABLE memories (
  id      TEXT PRIMARY KEY,
  bot_id  TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
  kind    TEXT NOT NULL DEFAULT 'fact',
  scope   TEXT NOT NULL DEFAULT 'bot',
  person  TEXT NOT NULL DEFAULT '',
  content TEXT NOT NULL,
  created INTEGER NOT NULL,
  updated INTEGER NOT NULL
);
CREATE INDEX memories_bot ON memories(bot_id);

-- One private skill library per account; `packaged` ones come from the
-- Marketplace. `bot_skills` holds per-Bot enablement.
CREATE TABLE skills (
  id           TEXT PRIMARY KEY,
  name         TEXT NOT NULL,
  description  TEXT NOT NULL DEFAULT '',
  instructions TEXT NOT NULL DEFAULT '',
  source       TEXT NOT NULL DEFAULT 'written',
  packaged     INTEGER NOT NULL DEFAULT 0,
  team         INTEGER NOT NULL DEFAULT 0,
  created      INTEGER NOT NULL,
  updated      INTEGER NOT NULL
);
CREATE UNIQUE INDEX skills_name ON skills(lower(name));

CREATE TABLE bot_skills (
  bot_id   TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
  skill_id TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
  PRIMARY KEY (bot_id, skill_id)
);

-- Routines. trigger: schedule | interval | webhook | slack | github | linear |
-- sentry | pagerduty | email. `schedule` is cron (schedule), minutes
-- (interval), or empty; `filter` is trigger-specific JSON.
CREATE TABLE routines (
  id          TEXT PRIMARY KEY,
  bot_id      TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
  name        TEXT NOT NULL,
  instruction TEXT NOT NULL,
  trigger     TEXT NOT NULL DEFAULT 'schedule',
  schedule    TEXT NOT NULL DEFAULT '',
  filter      TEXT NOT NULL DEFAULT '{}',
  webhook_key TEXT NOT NULL DEFAULT '',
  active      INTEGER NOT NULL DEFAULT 1,
  last_run    INTEGER,
  next_run    INTEGER,
  created     INTEGER NOT NULL,
  updated     INTEGER NOT NULL
);

CREATE TABLE runs (
  id         TEXT PRIMARY KEY,
  bot_id     TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
  chat_id    TEXT REFERENCES chats(id) ON DELETE SET NULL,
  routine_id TEXT REFERENCES routines(id) ON DELETE CASCADE,
  origin     TEXT NOT NULL DEFAULT 'chat',
  status     TEXT NOT NULL DEFAULT 'running',
  summary    TEXT NOT NULL DEFAULT '',
  error      TEXT NOT NULL DEFAULT '',
  steps      INTEGER NOT NULL DEFAULT 0,
  tokens     INTEGER NOT NULL DEFAULT 0,
  started    INTEGER NOT NULL,
  finished   INTEGER
);
CREATE INDEX runs_bot ON runs(bot_id, started);
CREATE INDEX runs_routine ON runs(routine_id, started);

-- kind: approval | review (raised by Auto-review) | local (local-computer
-- command) | connector. `interactive` approvals wait forever; others expire.
CREATE TABLE approvals (
  id          TEXT PRIMARY KEY,
  bot_id      TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
  chat_id     TEXT REFERENCES chats(id) ON DELETE CASCADE,
  run_id      TEXT,
  message_id  TEXT,
  kind        TEXT NOT NULL DEFAULT 'approval',
  tool        TEXT NOT NULL,
  target      TEXT NOT NULL DEFAULT '',
  args        TEXT NOT NULL DEFAULT '{}',
  reason      TEXT NOT NULL DEFAULT '',
  interactive INTEGER NOT NULL DEFAULT 1,
  status      TEXT NOT NULL DEFAULT 'pending',
  created     INTEGER NOT NULL,
  decided     INTEGER
);

-- Auto-review rules. kind: ask | allow. `locked` rows come from a team.
CREATE TABLE rules (
  id      TEXT PRIMARY KEY,
  kind    TEXT NOT NULL,
  text    TEXT NOT NULL,
  locked  INTEGER NOT NULL DEFAULT 0,
  created INTEGER NOT NULL
);

-- Per-Bot secrets: names and descriptions only; values live in the keychain.
CREATE TABLE secrets (
  bot_id      TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
  name        TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  created     INTEGER NOT NULL,
  updated     INTEGER NOT NULL,
  PRIMARY KEY (bot_id, name)
);

-- Installed plugins (connectors). kind: oauth | token | http | command.
-- `disabled` is a JSON array of tool names switched off.
CREATE TABLE plugins (
  id       TEXT PRIMARY KEY,
  catalog  TEXT NOT NULL DEFAULT '',
  name     TEXT NOT NULL,
  kind     TEXT NOT NULL,
  config   TEXT NOT NULL DEFAULT '{}',
  disabled TEXT NOT NULL DEFAULT '[]',
  status   TEXT NOT NULL DEFAULT 'added',
  created  INTEGER NOT NULL
);

-- Accounts connected to a plugin, each with a label ("work", "personal").
-- Tokens live in the keychain under `account-<id>`.
CREATE TABLE accounts (
  id        TEXT PRIMARY KEY,
  plugin_id TEXT NOT NULL REFERENCES plugins(id) ON DELETE CASCADE,
  label     TEXT NOT NULL DEFAULT '',
  identity  TEXT NOT NULL DEFAULT '',
  status    TEXT NOT NULL DEFAULT 'connected',
  created   INTEGER NOT NULL
);

-- Shareable Bot templates. visibility: public | team.
CREATE TABLE templates (
  id         TEXT PRIMARY KEY,
  bot_id     TEXT REFERENCES bots(id) ON DELETE SET NULL,
  name       TEXT NOT NULL,
  visibility TEXT NOT NULL DEFAULT 'public',
  payload    TEXT NOT NULL,
  created    INTEGER NOT NULL,
  updated    INTEGER NOT NULL
);

CREATE TABLE notifications (
  id      TEXT PRIMARY KEY,
  bot_id  TEXT REFERENCES bots(id) ON DELETE CASCADE,
  chat_id TEXT REFERENCES chats(id) ON DELETE CASCADE,
  kind    TEXT NOT NULL DEFAULT 'info',
  title   TEXT NOT NULL,
  body    TEXT NOT NULL DEFAULT '',
  read    INTEGER NOT NULL DEFAULT 0,
  created INTEGER NOT NULL
);

CREATE TABLE usage (
  day               TEXT NOT NULL,
  model             TEXT NOT NULL,
  bot_id            TEXT NOT NULL DEFAULT '',
  prompt_tokens     INTEGER NOT NULL DEFAULT 0,
  completion_tokens INTEGER NOT NULL DEFAULT 0,
  requests          INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (day, model, bot_id)
);

-- Key/value app state that is data, not preference (onboarding progress,
-- last-seen activity for the inactivity check).
CREATE TABLE state (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
