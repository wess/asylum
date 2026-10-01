//! The app manifest a Team Bot's Slack app is created from: its name, a bot
//! user, Socket Mode, and the events and scopes it needs.

use serde_json::{json, Value};

pub const SCOPES: [&str; 9] = [
  "app_mentions:read",
  "chat:write",
  "im:history",
  "im:read",
  "im:write",
  "channels:history",
  "groups:history",
  "mpim:history",
  "users:read",
];

pub const EVENTS: [&str; 5] = ["app_mention", "message.im", "message.channels", "message.groups", "message.mpim"];

pub fn build(name: &str, description: &str) -> Value {
  let display: String = name.chars().take(35).collect();
  json!({
    "display_information": {
      "name": display,
      "description": description.chars().take(139).collect::<String>(),
      "background_color": "#2a1866"
    },
    "features": {
      "app_home": { "messages_tab_enabled": true, "messages_tab_read_only_enabled": false },
      "bot_user": { "display_name": display, "always_online": true }
    },
    "oauth_config": { "scopes": { "bot": SCOPES } },
    "settings": {
      "event_subscriptions": { "bot_events": EVENTS },
      "interactivity": { "is_enabled": false },
      "socket_mode_enabled": true,
      "token_rotation_enabled": false
    }
  })
}

/// Slack's "create an app from this manifest" link.
pub fn create_url(manifest: &Value) -> String {
  format!("https://api.slack.com/apps?new_app=1&manifest_json={}", urlencoding::encode(&manifest.to_string()))
}

#[cfg(test)]
#[path = "../tests/manifest.rs"]
mod tests;
