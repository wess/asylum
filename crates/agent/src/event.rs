//! What the runtime tells the UI. Broadcast to every subscriber; the UI
//! re-reads the store for whatever changed.

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
  BotsChanged,
  ChatsChanged,
  Message { chat: String, message: String },
  /// A streaming message changed; `text` is the latest body.
  Stream { chat: String, message: String, text: String },
  BotStatus { bot: String, status: String },
  Approval { id: String },
  Notify { bot: Option<String>, chat: Option<String>, title: String, body: String },
  Screen { bot: String },
  Routines { bot: String },
  Skills,
  Plugins,
  /// An in-app error notice for a chat.
  Notice { chat: Option<String>, text: String, request: String },
  Usage,
  Computer { state: String },
}
