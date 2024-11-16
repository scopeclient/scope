use gpui::{ListState, Pixels};
use scope_backend_discord::{
  message::{
    author::{DiscordMessageAuthor, DisplayName},
    content::DiscordMessageContent,
    DiscordMessage,
  },
  snowflake::Snowflake,
};
use scope_chat::message::Message;

#[derive(Clone)]
pub struct MessageList<M: Message + 'static> {
  real_messages: Vec<M>,
  pending_messages: Vec<M>,
}

impl<M: Message> MessageList<M> {
  pub fn new() -> MessageList<M> {
    Self {
      real_messages: Vec::default(),
      pending_messages: Vec::default(),
    }
  }

  pub fn add_external_message(&mut self, message: M) {
    self.real_messages.push(message);
  }

  pub fn add_pending_message(&mut self, pending_message: M) {
    self.pending_messages.push(pending_message);
  }

  pub fn length(&self) -> usize {
    self.real_messages.len() + self.pending_messages.len()
  }

  pub fn get(&self, index: usize) -> Option<&M> {
    if index >= self.pending_messages.len() {
      self.real_messages.get(index - self.pending_messages.len())
    } else {
      self.pending_messages.get(index)
    }
  }

  pub fn create_list_state(&self) -> ListState {
    let clone = self.clone();

    ListState::new(
      clone.length(),
      ListAlignment::Bottom,
      Pixels(20.),
      move |idx, _cx| {
        let item = clone.get(idx).unwrap().clone();
        div().child(message(item)).into_any_element()
      },
    )
  }
}
