use serenity::all::{ChannelId, GuildId, MessageId, UserId};

#[derive(Clone, Hash, PartialEq, Eq, Copy, Debug)]
pub struct Snowflake(pub u64);

impl Snowflake {
  pub fn random() -> Snowflake {
    Snowflake(rand::random())
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
