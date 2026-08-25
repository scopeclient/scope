//! Application state shared by every panel: connection, navigation data, tabs.
//!
//! Panels hold an `Entity<AppState>`, `observe` it for re-renders, and call
//! the mutation methods below from their click handlers.

use std::{collections::HashMap, sync::Arc};

use gpui::{AnyView, AppContext as _, Context, Entity, Window};
use scope_backend_demo::DemoClient;
use scope_backend_discord::client::{ConnectError, DiscordClient, Intents, TokenKind};
use scope_chat::{
  event::ClientEvent,
  nav::{ChannelInfo, ChannelKind, GuildInfo, Id, MemberInfo, UserInfo},
};
use tokio::sync::broadcast::error::RecvError;

use crate::{
  backend::Backend,
  channel::{ChannelViewOptions, typing::TypingIndicator},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tab {
  pub channel: Id,
  pub guild: Option<Id>,
  /// Display title, e.g. `#announcements` or a DM partner's name.
  pub title: String,
  pub icon_url: Option<String>,
}

#[derive(Default)]
pub enum Connection {
  #[default]
  Disconnected,
  Connecting,
  Connected(Arc<dyn Backend>),
  Failed(String),
}

#[derive(Default)]
pub struct AppState {
  pub connection: Connection,

  pub user: Option<UserInfo>,
  pub guilds: Vec<GuildInfo>,
  pub dms: Vec<ChannelInfo>,
  /// Channels per guild.
  pub channels: HashMap<Id, Vec<ChannelInfo>>,
  /// Members per guild.
  pub members: HashMap<Id, Vec<MemberInfo>>,

  pub selected_guild: Option<Id>,
  pub tabs: Vec<Tab>,
  pub active_tab: Option<usize>,

  pub show_channel_nav: bool,
  pub show_member_list: bool,

  /// Non-fatal warning to surface to the user (e.g. intents disabled).
  pub notice: Option<String>,
  /// Display name from token validation, shown while connecting.
  pub connecting_as: Option<String>,

  /// Chat views, created lazily per channel and kept alive across tab switches.
  pub channel_views: HashMap<Id, AnyView>,
  /// Who is typing, per channel; shared with the channel's composer.
  pub typing: HashMap<Id, Entity<TypingIndicator>>,
}

impl AppState {
  pub fn new() -> Self {
    AppState {
      show_channel_nav: true,
      show_member_list: true,
      ..Default::default()
    }
  }

  // ---- queries ------------------------------------------------------------

  pub fn backend(&self) -> Option<&Arc<dyn Backend>> {
    match &self.connection {
      Connection::Connected(backend) => Some(backend),
      _ => None,
    }
  }

  pub fn is_connected(&self) -> bool {
    self.backend().is_some()
  }

  /// True when running against the offline sample backend (`SCOPE_DEMO=1`).
  pub fn is_demo(&self) -> bool {
    self.backend().is_some_and(|b| b.is_demo())
  }

  pub fn active_tab(&self) -> Option<&Tab> {
    self.active_tab.and_then(|i| self.tabs.get(i))
  }

  pub fn active_channel(&self) -> Option<Id> {
    self.active_tab().map(|t| t.channel)
  }

  pub fn active_channel_view(&self) -> Option<AnyView> {
    self.active_channel().and_then(|id| self.channel_views.get(&id).cloned())
  }

  pub fn guild(&self, id: Id) -> Option<&GuildInfo> {
    self.guilds.iter().find(|g| g.id == id)
  }

  pub fn selected_guild_info(&self) -> Option<&GuildInfo> {
    self.selected_guild.and_then(|id| self.guild(id))
  }

  pub fn channel_info(&self, id: Id) -> Option<&ChannelInfo> {
    self.channels.values().flatten().chain(self.dms.iter()).find(|c| c.id == id)
  }

  pub fn selected_guild_channels(&self) -> &[ChannelInfo] {
    self.selected_guild.and_then(|g| self.channels.get(&g)).map(Vec::as_slice).unwrap_or(&[])
  }

  pub fn selected_guild_members(&self) -> &[MemberInfo] {
    self.selected_guild.and_then(|g| self.members.get(&g)).map(Vec::as_slice).unwrap_or(&[])
  }

  // ---- connection ---------------------------------------------------------

  /// Connect with a Discord token. Progress is reported through `connection`.
  ///
  /// Bots whose privileged intents are disabled are retried without them, so
  /// they still connect (with a thinner member list / no presence).
  /// `persist` manages the stored login: save the token once the gateway is
  /// ready, forget it if Discord rejects it. Env-provided tokens pass `false`.
  pub fn connect(&mut self, token: String, kind: TokenKind, persist: bool, cx: &mut Context<Self>) {
    if matches!(self.connection, Connection::Connecting) {
      return;
    }

    self.connection = Connection::Connecting;
    self.notice = None;
    self.connecting_as = None;
    cx.notify();

    cx.spawn(async move |this, cx| {
      let mut notice = None;

      // Cheap users/@me check first: catches bad tokens without a gateway
      // round-trip and gives us a name for the "connecting as …" state.
      match DiscordClient::validate_token(&token, kind).await {
        Ok(name) => {
          this
            .update(cx, |this, cx| {
              this.connecting_as = Some(name);
              cx.notify();
            })
            .ok();
        }
        Err(error) => {
          if persist && error == ConnectError::InvalidToken {
            crate::auth::forget();
          }
          this
            .update(cx, |this, cx| {
              this.connection = Connection::Failed(error.to_string());
              cx.notify();
            })
            .ok();
          return;
        }
      }

      let result = match DiscordClient::new(token.clone(), kind, Intents::All).await {
        Err(ConnectError::DisallowedIntents) if kind == TokenKind::Bot => {
          log::warn!("privileged intents are disabled for this bot; retrying without them");
          notice = Some(ConnectError::DisallowedIntents.to_string());
          DiscordClient::new(token.clone(), kind, Intents::NonPrivileged).await
        }
        other => other,
      };

      match result {
        Ok(client) => {
          if persist {
            crate::auth::save(&token, kind);
          }

          this
            .update(cx, |this, cx| {
              this.notice = notice;
              this.set_backend(client, cx);
            })
            .ok();
        }
        Err(error) => {
          if persist && error == ConnectError::InvalidToken {
            crate::auth::forget();
          }

          this
            .update(cx, |this, cx| {
              this.connection = Connection::Failed(error.to_string());
              cx.notify();
            })
            .ok();
        }
      }
    })
    .detach();
  }

  /// Use the offline sample backend and open the channels the mockup shows.
  pub fn connect_demo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.set_backend(DemoClient::new(), cx);

    // Mirror the mockup (Prism AIO's #announcements, Wrath's #member-important)
    // and land on #dev-announcements, which showcases every rich message kind.
    self.open_channel(Id(102), window, cx);
    self.open_channel(Id(1101), window, cx);
    self.open_channel(Id(103), window, cx);
    self.activate_tab(2, cx);
  }

  fn set_backend(&mut self, backend: Arc<impl Backend>, cx: &mut Context<Self>) {
    let backend: Arc<dyn Backend> = backend;
    log::info!("connected to {} backend", backend.name());

    self.connection = Connection::Connected(backend.clone());
    self.refresh_from_backend(cx);
    self.pump_events(backend, cx);
  }

  /// Forward backend events (tokio) onto the gpui thread and refresh nav data.
  fn pump_events(&self, backend: Arc<dyn Backend>, cx: &mut Context<Self>) {
    let mut events = backend.events();

    cx.spawn(async move |this, cx| {
      loop {
        let (tx, rx) = catty::oneshot();

        tokio::spawn(async move {
          let result = events.recv().await;
          let _ = tx.send((result, events));
        });

        let Ok((result, returned)) = rx.await else { break };
        events = returned;

        let alive = match result {
          Ok(ClientEvent::Ready | ClientEvent::GuildsUpdated) | Err(RecvError::Lagged(_)) => {
            this.update(cx, |this, cx| this.refresh_from_backend(cx))
          }
          Ok(ClientEvent::ChannelsUpdated(guild)) => this.update(cx, |this, cx| this.refresh_guild_channels(guild, cx)),
          // Presence/member chatter arrives constantly from every guild on a
          // real account; only the selected guild is on screen, so only it is
          // worth refreshing (selecting a guild refreshes it on entry).
          Ok(ClientEvent::MembersUpdated(guild) | ClientEvent::PresenceUpdated(guild)) => {
            this.update(cx, |this, cx| this.refresh_guild_members(guild, cx))
          }
          Ok(ClientEvent::Typing { channel, user }) => this.update(cx, |this, cx| {
            this.typing_for(channel, cx).update(cx, |typing, cx| typing.started(user, cx));
          }),
          Err(RecvError::Closed) => break,
        };

        if alive.is_err() {
          break;
        }
      }
    })
    .detach();
  }

  /// Re-read everything navigation-related from the backend.
  pub fn refresh_from_backend(&mut self, cx: &mut Context<Self>) {
    let Some(backend) = self.backend().cloned() else { return };

    self.user = Some(backend.current_user());
    self.guilds = backend.guilds();
    self.dms = backend.dm_channels();

    if self.selected_guild.is_none_or(|g| self.guild(g).is_none()) {
      self.selected_guild = self.guilds.first().map(|g| g.id);
    }

    // Only the guild on screen needs its channel/member details; the rest are
    // fetched when selected. Keeps a GuildsUpdated burst from cloning the world.
    if let Some(guild) = self.selected_guild {
      self.channels.insert(guild, backend.guild_channels(guild));
      self.members.insert(guild, backend.guild_members(guild));
    }

    cx.notify();
  }

  pub fn refresh_guild_channels(&mut self, guild: Id, cx: &mut Context<Self>) {
    let Some(backend) = self.backend().cloned() else { return };
    let fresh = backend.guild_channels(guild);

    if self.channels.get(&guild) != Some(&fresh) {
      self.channels.insert(guild, fresh);
      cx.notify();
    }
  }

  /// Ignores guilds that are not on screen and skips the re-render when the
  /// member list comes back unchanged.
  pub fn refresh_guild_members(&mut self, guild: Id, cx: &mut Context<Self>) {
    if self.selected_guild != Some(guild) {
      return;
    }

    let Some(backend) = self.backend().cloned() else { return };
    let fresh = backend.guild_members(guild);

    if self.members.get(&guild) != Some(&fresh) {
      self.members.insert(guild, fresh);
      cx.notify();
    }
  }

  // ---- navigation ---------------------------------------------------------

  pub fn select_guild(&mut self, guild: Id, cx: &mut Context<Self>) {
    self.selected_guild = Some(guild);

    // Entering a guild pulls its channels and members fresh; while it stays
    // selected, events keep it up to date.
    if let Some(backend) = self.backend().cloned() {
      self.channels.insert(guild, backend.guild_channels(guild));
      self.members.insert(guild, backend.guild_members(guild));
    }

    cx.notify();
  }

  /// Open a channel in a tab (activating the existing tab if there is one).
  pub fn open_channel(&mut self, channel: Id, window: &mut Window, cx: &mut Context<Self>) {
    let Some(info) = self.channel_info(channel).cloned() else { return };

    if !info.kind.is_messageable() {
      return;
    }

    if let Some(index) = self.tabs.iter().position(|t| t.channel == channel) {
      self.active_tab = Some(index);
    } else {
      let title = match info.kind {
        ChannelKind::DirectMessage | ChannelKind::GroupDm => info.name.clone(),
        _ => format!("#{}", info.name),
      };
      let icon_url = info.guild_id.and_then(|g| self.guild(g)).and_then(|g| g.icon_url.clone()).or_else(|| info.icon_url.clone());

      self.tabs.push(Tab {
        channel,
        guild: info.guild_id,
        title,
        icon_url,
      });
      self.active_tab = Some(self.tabs.len() - 1);
    }

    if let Some(guild) = info.guild_id {
      self.selected_guild = Some(guild);
    }

    self.ensure_channel_view(channel, window, cx);
    cx.notify();
  }

  pub fn activate_tab(&mut self, index: usize, cx: &mut Context<Self>) {
    if index < self.tabs.len() {
      self.active_tab = Some(index);

      if let Some(guild) = self.tabs[index].guild {
        self.selected_guild = Some(guild);
      }

      cx.notify();
    }
  }

  pub fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
    if index >= self.tabs.len() {
      return;
    }

    self.tabs.remove(index);

    self.active_tab = match self.active_tab {
      None => None,
      Some(_) if self.tabs.is_empty() => None,
      Some(active) if active > index => Some(active - 1),
      Some(active) if active == index => Some(index.min(self.tabs.len() - 1)),
      Some(active) => Some(active),
    };

    cx.notify();
  }

  pub fn toggle_channel_nav(&mut self, cx: &mut Context<Self>) {
    self.show_channel_nav = !self.show_channel_nav;
    cx.notify();
  }

  pub fn toggle_member_list(&mut self, cx: &mut Context<Self>) {
    self.show_member_list = !self.show_member_list;
    cx.notify();
  }

  /// Typing state for a channel, created on first use.
  pub fn typing_for(&mut self, channel: Id, cx: &mut Context<Self>) -> Entity<TypingIndicator> {
    self.typing.entry(channel).or_insert_with(|| cx.new(|_| TypingIndicator::default())).clone()
  }

  fn ensure_channel_view(&mut self, channel: Id, window: &mut Window, cx: &mut Context<Self>) {
    if self.channel_views.contains_key(&channel) {
      return;
    }

    let Some(backend) = self.backend().cloned() else { return };

    let options = ChannelViewOptions {
      title: self
        .tabs
        .iter()
        .find(|t| t.channel == channel)
        .map(|t| t.title.clone())
        .or_else(|| self.channel_info(channel).map(|c| format!("#{}", c.name)))
        .unwrap_or_default(),
      composer_avatar: self.user.as_ref().and_then(|u| u.avatar_url.clone()),
      typing: Some(self.typing_for(channel, cx)),
    };

    let task = backend.open_channel(channel, options, window, cx);

    cx.spawn(async move |this, cx| {
      if let Some(view) = task.await {
        this
          .update(cx, |this, cx| {
            this.channel_views.insert(channel, view);
            cx.notify();
          })
          .ok();
      }
    })
    .detach();
  }
}
