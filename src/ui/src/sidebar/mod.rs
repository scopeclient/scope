use crate::sidebar::channel_tree::ChannelTree;
use gpui::prelude::FluentBuilder;
use gpui::{div, IntoElement, ParentElement, Render, Styled, View, ViewContext, VisualContext, WindowContext};
use scope_backend_discord::channel::DiscordChannelMetadata;
use scope_backend_discord::client::DiscordClient;
use scope_chat::channel::ChannelMetadata;
use std::sync::Arc;
use components::theme::ActiveTheme;
use crate::channel::ChannelView;

mod channel_tree;

pub struct Sidebar<ChannelMeta: ChannelMetadata + 'static> {
  channel_tree: Option<View<ChannelTree<ChannelMeta>>>,
}

impl Sidebar<DiscordChannelMetadata> {
  pub fn create(cx: &mut ViewContext<'_, Sidebar<DiscordChannelMetadata>>, client: Option<Arc<DiscordClient>>) -> Self {
    Sidebar {
      channel_tree: if let Some(client) = client {
        Some(ChannelTree::new(cx, client))
      } else {
        None
      },
    }
  }
}

impl<ChannelMeta: ChannelMetadata + 'static> Render for Sidebar<ChannelMeta> {
  fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
    div()
        .min_w_64()
        .h_full()
        .text_color(cx.theme().foreground)
        .bg(cx.theme().panel)
        .child("» Private Messages")
        .when_some(self.channel_tree.clone(), |div, view| div.child(view))
  }
}
