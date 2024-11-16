use gpui::{div, IntoElement, ListAlignment, ListState, ParentElement, Pixels};

use scope_chat::message::Message;

use super::message::message;

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
    if let Some((_, pending_index)) = self
      .pending_messages
      .iter()
      .zip(0..)
      .find(|(msg, _)| msg.get_nonce().map(|v1| message.get_nonce().map(|v2| v2 == v1).unwrap_or(false)).unwrap_or(false))
    {
      self.pending_messages.remove(pending_index);
    }

    self.real_messages.push(message);
  }

  pub fn add_pending_message(&mut self, pending_message: M) {
    self.pending_messages.push(pending_message);
  }

  pub fn length(&self) -> usize {
    self.real_messages.len() + self.pending_messages.len()
  }

  pub fn get(&self, index: usize) -> Option<&M> {
    if index >= self.real_messages.len() {
      self.pending_messages.get(index - self.real_messages.len())
    } else {
      self.real_messages.get(index)
    }
  }

  pub fn create_list_state(&self) -> ListState {
    let clone = self.clone();

    ListState::new(clone.length(), ListAlignment::Bottom, Pixels(20.), move |idx, _cx| {
      let item = clone.get(idx).unwrap().clone();
      div().child(message(item)).into_any_element()
    })
  }
}
