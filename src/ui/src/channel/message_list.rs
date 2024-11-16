use gpui::{div, IntoElement, ListAlignment, ListState, ParentElement, Pixels};

use scope_chat::message::{Message, MessageAuthor};

use super::message::{message, MessageGroup};

#[derive(Clone)]
pub struct MessageList<M: Message + 'static> {
  messages: Vec<MessageGroup<M>>,
}

impl<M: Message> MessageList<M> {
  pub fn new() -> MessageList<M> {
    Self { messages: Vec::default() }
  }

  pub fn add_external_message(&mut self, message: M) {
    if let Some(nonce) = message.get_nonce() {
      let mut removal_index: Option<usize> = None;

      for (group, index) in self.messages.iter_mut().zip(0..) {
        let matching = group.find_matching(nonce);

        if let Some(matching) = matching {
          if group.size() == 1 {
            removal_index = Some(index);
          } else {
            group.remove(matching);
          }
        }
      }

      if let Some(removal_index) = removal_index {
        self.messages.remove(removal_index);
      }
    }

    let last = self.messages.last_mut();

    if last.is_some()
      && last.as_ref().unwrap().get_author().get_id() == message.get_author().get_id()
      && message.should_group(last.as_ref().unwrap().last())
    {
      last.unwrap().add(message);
    } else {
      self.messages.push(MessageGroup::new(message));
    }
  }

  pub fn add_pending_message(&mut self, pending_message: M) {
    let last = self.messages.last_mut();

    if last.is_some()
      && last.as_ref().unwrap().get_author().get_id() == pending_message.get_author().get_id()
      && pending_message.should_group(last.as_ref().unwrap().last())
    {
      last.unwrap().add(pending_message);
    } else {
      self.messages.push(MessageGroup::new(pending_message));
    }
  }

  pub fn length(&self) -> usize {
    self.messages.len()
  }

  pub fn get(&self, index: usize) -> Option<&MessageGroup<M>> {
    self.messages.get(index)
  }

  pub fn create_list_state(&self) -> ListState {
    let clone = self.clone();

    ListState::new(clone.length(), ListAlignment::Bottom, Pixels(20.), move |idx, _cx| {
      let item = clone.get(idx).unwrap().clone();
      div().child(message(item)).into_any_element()
    })
  }
}
