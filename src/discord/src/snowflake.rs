use serenity::all::{ChannelId, GuildId, MessageId, UserId};

#[derive(Clone, Hash, PartialEq, Eq, Copy, Debug)]
pub struct Snowflake(pub u64);

impl Snowflake {
  /// A plausible, definitely-unused id for optimistic (pending) messages.
  ///
  /// Shaped like a real snowflake — milliseconds since the Discord epoch in
  /// the high bits — so it stays below Discord's `i64::MAX` ceiling (a raw
  /// `u64` here once leaked into a `?before=` query and got a 400) and sorts
  /// correctly against real ids.
  pub fn random() -> Snowflake {
    const DISCORD_EPOCH_MS: i64 = 1_420_070_400_000;
    let ms = (chrono::Utc::now().timestamp_millis() - DISCORD_EPOCH_MS).max(0) as u64;
    Snowflake((ms << 22) | (rand::random::<u64>() & 0x3F_FFFF))
  }
}

#[allow(clippy::to_string_trait_impl)]
impl ToString for Snowflake {
  fn to_string(&self) -> String {
    self.0.to_string()
  }
}

impl From<UserId> for Snowflake {
  fn from(value: UserId) -> Self {
    Snowflake(value.get())
  }
}

impl From<GuildId> for Snowflake {
  fn from(value: GuildId) -> Self {
    Snowflake(value.get())
  }
}

impl From<ChannelId> for Snowflake {
  fn from(value: ChannelId) -> Self {
    Snowflake(value.get())
  }
}

impl From<MessageId> for Snowflake {
  fn from(value: MessageId) -> Self {
    Snowflake(value.get())
  }
}

impl From<Snowflake> for scope_chat::nav::Id {
  fn from(value: Snowflake) -> Self {
    scope_chat::nav::Id(value.0)
  }
}

impl From<scope_chat::nav::Id> for Snowflake {
  fn from(value: scope_chat::nav::Id) -> Self {
    Snowflake(value.0)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn random_snowflakes_fit_discords_ceiling() {
    for _ in 0..1000 {
      assert!(Snowflake::random().0 <= i64::MAX as u64);
    }
  }
}
