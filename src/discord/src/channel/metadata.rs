use crate::client::DiscordClient;
use crate::snowflake::Snowflake;
use scope_chat::channel::ChannelMetadata;
use serenity::all::Channel;
use std::sync::Arc;

#[derive(Clone)]
pub struct DiscordChannelMetadata {
  client: Arc<DiscordClient>,
  id: Snowflake,
  channel: Option<Channel>,
  name: Option<String>,
  icon: Option<String>,
}

impl ChannelMetadata for DiscordChannelMetadata {
  fn get_name(&self) -> String {
    if let Some(name) = &self.name {
      return name.clone();
    }
    match &self.channel {
      Some(Channel::Guild(guild_channel)) => guild_channel.name.clone(),
      Some(Channel::Private(private_channel)) => private_channel.name(),
      None => unreachable!("Either name or channel should be set"),
      other => {
        unreachable!(
          "According to the code, there are only two types of channels: Guild and Private. Got: {:?}",
          other
        )
      }
    }
  }

  fn get_id(&self) -> String {
    self.id.to_string()
  }

  fn get_icon(&self) -> Option<String> {
    self.icon.clone()
  }
}

impl DiscordChannelMetadata {
  pub async fn create(client: Arc<DiscordClient>, id: Snowflake) -> Option<Self> {
    let channel = client.get_channel(id).await?;
    Some(DiscordChannelMetadata {
      id,
      client,
      channel: Some(channel),
      name: None,
      icon: None,
    })
  }

  pub async fn new(client: Arc<DiscordClient>, id: Snowflake, name: String, icon: String) -> Self {
    DiscordChannelMetadata {
      id,
      client,
      channel: None,
      name: Some(name),
      icon: Some(icon),
    }
  }
}
