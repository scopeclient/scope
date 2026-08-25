//! Regression: a bot's READY omits the user-only fields the serenity fork's
//! `Ready` required (`user_settings_proto`, `connected_accounts`,
//! `friend_suggestion_count`). Before the vendored `#[serde(default)]` fix this
//! failed to deserialize, so serenity dropped READY and Scope hung on login.

use serenity::model::gateway::Ready;

#[test]
fn bot_ready_without_user_only_fields_deserializes() {
  let payload = serde_json::json!({
    "v": 10,
    "user": {
      "id": "111111111111111111", "username": "scope-test-bot", "discriminator": "0001",
      "avatar": null, "bot": true, "mfa_enabled": false, "verified": true, "email": null,
      "flags": 0, "public_flags": 0
    },
    "guilds": [{ "id": "222222222222222222", "unavailable": true }],
    "session_id": "abc123",
    "resume_gateway_url": "wss://gateway.discord.gg",
    "shard": [0, 1],
    "application": { "id": "111111111111111111", "flags": 0 }
  });

  let ready: Ready = serde_json::from_value(payload).expect("bot READY should deserialize");
  assert_eq!(ready.version, 10);
  assert_eq!(ready.guilds.len(), 1);
}
