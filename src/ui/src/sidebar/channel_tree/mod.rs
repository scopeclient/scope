use crate::sidebar::channel_tree::channel_entry::ChannelEntry;
use gpui::{div, Context, IntoElement, Model, ParentElement, Render, View, ViewContext, VisualContext, WindowContext};
use scope_backend_discord::channel::DiscordChannelMetadata;
use scope_backend_discord::client::DiscordClient;
use scope_chat::channel::ChannelMetadata;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;

mod channel_entry;

pub struct ChannelTree<Meta: ChannelMetadata + 'static> {
  entries: Model<Vec<Arc<Meta>>>,
}

impl ChannelTree<DiscordChannelMetadata> {
  pub fn new(cx: &mut WindowContext, client: Arc<DiscordClient>) -> View<Self> {
    cx.new_view(|cx| {
      let entries_model = cx.new_model(|_cx| vec![]);
      let async_entries_model = entries_model.clone();
      let mut async_cx = cx.to_async();
      let (sender, mut receiver) = broadcast::channel::<DiscordChannelMetadata>(10);
      cx.foreground_executor()
        .spawn(async move {
          client.set_channel_update_sender(sender).await;
          loop {
            if let Ok(_) = receiver.recv().await {
              let new_entries = client.list_direct_message_channels().await;
              let _ = async_entries_model.update(&mut async_cx, |entries, _cx| {
                *entries = new_entries.iter().map(|entry| Arc::new(entry.clone())).collect();
              });
            }
          }
        })
        .detach();

      ChannelTree { entries: entries_model }
    })
  }
}

impl<Meta: ChannelMetadata + 'static> Render for ChannelTree<Meta> {
  fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
    let entries = self.entries.read(cx);
    div().children(entries.iter().map(|entry| ChannelEntry::new(entry.clone())))
  }
}
