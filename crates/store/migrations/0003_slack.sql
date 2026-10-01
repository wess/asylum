-- A Team Bot's Slack app. Tokens live in the Keychain.
CREATE TABLE slack_apps (
  bot_id   TEXT PRIMARY KEY REFERENCES bots(id) ON DELETE CASCADE,
  team     TEXT NOT NULL DEFAULT '',
  bot_user TEXT NOT NULL DEFAULT '',
  status   TEXT NOT NULL DEFAULT 'connected',
  created  INTEGER NOT NULL
);

-- Teammates whose Slack accounts are linked; others are asked to link.
CREATE TABLE slack_links (
  slack_user TEXT PRIMARY KEY,
  name       TEXT NOT NULL DEFAULT '',
  created    INTEGER NOT NULL
);

-- Which Asylum chat a Slack DM or thread maps to, and threads the Bot follows.
CREATE TABLE slack_chats (
  bot_id   TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
  key      TEXT NOT NULL,
  chat_id  TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
  follow   INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (bot_id, key)
);
