//! The socket every chat backend plugs into. The UI only ever talks to
//! `Arc<dyn Backend>`, so Discord and the offline demo are interchangeable.

use std::{future::Future, sync::Arc};

use gpui::{AnyView, App, AppContext as _, Task, Window};
use scope_backend_demo::DemoClient;
use scope_backend_discord::{client::DiscordClient, snowflake::Snowflake};
use scope_chat::{
  channel::Channel,
  event::ClientEvent,
  nav::{ChannelInfo, GuildInfo, Id, MemberInfo, UserInfo},
};
use tokio::sync::broadcast;

use crate::channel::{ChannelView, ChannelViewOptions};

pub trait Backend: Send + Sync + 'static {
  fn name(&self) -> &'static str;

  /// Offline sample backend — panels may show mockup-only decorations.
  fn is_demo(&self) -> bool {
    false
  }

  fn current_user(&self) -> UserInfo;
  fn guilds(&self) -> Vec<GuildInfo>;
  fn guild_channels(&self, guild: Id) -> Vec<ChannelInfo>;
  fn guild_members(&self, guild: Id) -> Vec<MemberInfo>;
  fn dm_channels(&self) -> Vec<ChannelInfo>;
  fn events(&self) -> broadcast::Receiver<ClientEvent>;

  /// Open a channel and build its chat view on the gpui foreground thread.
  fn open_channel(self: Arc<Self>, channel: Id, options: ChannelViewOptions, window: &mut Window, cx: &mut App) -> Task<Option<AnyView>>;
}

/// Await the backend's channel handle, then build a `ChannelView` for it.
fn build_channel_view<C: Channel + 'static>(
  channel: impl Future<Output = Arc<C>> + 'static,
  options: ChannelViewOptions,
  window: &mut Window,
  cx: &mut App,
) -> Task<Option<AnyView>> {
  window.spawn(cx, async move |cx| {
    let channel = channel.await;
    cx.update(|window, cx| cx.new(|cx| ChannelView::<C>::create(window, cx, channel, options)).into()).ok()
  })
}

impl Backend for DiscordClient {
  fn name(&self) -> &'static str {
    "discord"
  }

  fn current_user(&self) -> UserInfo {
    DiscordClient::current_user(self)
  }

  fn guilds(&self) -> Vec<GuildInfo> {
    DiscordClient::guilds(self)
  }

  fn guild_channels(&self, guild: Id) -> Vec<ChannelInfo> {
    DiscordClient::guild_channels(self, guild)
  }

  fn guild_members(&self, guild: Id) -> Vec<MemberInfo> {
    DiscordClient::guild_members(self, guild)
  }

  fn dm_channels(&self) -> Vec<ChannelInfo> {
    DiscordClient::dm_channels(self)
  }

  fn events(&self) -> broadcast::Receiver<ClientEvent> {
    DiscordClient::events(self)
  }

  fn open_channel(self: Arc<Self>, channel: Id, options: ChannelViewOptions, window: &mut Window, cx: &mut App) -> Task<Option<AnyView>> {
    build_channel_view(DiscordClient::channel(self, Snowflake::from(channel)), options, window, cx)
  }
}

impl Backend for DemoClient {
  fn name(&self) -> &'static str {
    "demo"
  }

  fn is_demo(&self) -> bool {
    true
  }

  fn current_user(&self) -> UserInfo {
    DemoClient::current_user(self)
  }

  fn guilds(&self) -> Vec<GuildInfo> {
    DemoClient::guilds(self)
  }

  fn guild_channels(&self, guild: Id) -> Vec<ChannelInfo> {
    DemoClient::guild_channels(self, guild)
  }

  fn guild_members(&self, guild: Id) -> Vec<MemberInfo> {
    DemoClient::guild_members(self, guild)
  }

  fn dm_channels(&self) -> Vec<ChannelInfo> {
    DemoClient::dm_channels(self)
  }

  fn events(&self) -> broadcast::Receiver<ClientEvent> {
    DemoClient::events(self)
  }

  fn open_channel(self: Arc<Self>, channel: Id, options: ChannelViewOptions, window: &mut Window, cx: &mut App) -> Task<Option<AnyView>> {
    build_channel_view(DemoClient::channel(self, channel), options, window, cx)
  }
}
