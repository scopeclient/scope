use gpui::{Render, View};
use scope_chat::{channel::Channel, message::Message};

pub struct ChannelView<M: Message> {
  channel: Box<dyn Channel<Message = M>>
}

impl<M: Message + 'static> Render for ChannelView<M> {
  fn render(&mut self, cx: &mut gpui::ViewContext<Self>) -> impl gpui::IntoElement {
    
  }
}