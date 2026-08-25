use std::{fmt::Debug, hash::Hash, sync::Arc};

use scope_rich::Emoji;
use tokio::sync::broadcast;

use crate::{
  async_list::{AsyncList, AsyncListItem},
  message::Message,
};

/// Live changes to a channel's messages, as pushed by the backend.
#[derive(Clone)]
pub enum ChannelEvent<M: AsyncListItem> {
  New(M),
  /// An existing message changed (edit, reactions, embeds resolved, …).
  Updated(M),
  Deleted(M::Identifier),
}

pub trait Channel: AsyncList<Content = Self::Message> + Send + Sync + Clone {
  type Message: Message<Identifier = Self::Identifier> + AsyncListItem<Identifier = Self::Identifier>;
  type Identifier: Sized + Copy + Clone + Debug + Eq + PartialEq + Send + Hash;

  fn get_receiver(&self) -> broadcast::Receiver<ChannelEvent<Self::Message>>;

  /// Send a message; returns the optimistic (pending) message immediately.
  fn send_message(&self, content: String, nonce: String) -> Self::Message;

  /// Send a message replying to `reply_to`; returns the optimistic message.
  fn send_reply(&self, content: String, nonce: String, reply_to: Self::Identifier) -> Self::Message;

  /// Replace the content of one of our own messages.
  fn edit_message(&self, message: Self::Identifier, content: String);

  fn delete_message(&self, message: Self::Identifier);

  fn add_reaction(&self, message: Self::Identifier, emoji: Emoji);

  fn remove_reaction(&self, message: Self::Identifier, emoji: Emoji);

  /// Tell the server the user is typing (call at most every few seconds).
  fn typing(&self);

  fn get_identifier(&self) -> Self::Identifier;
}

impl<C: Channel> Channel for Arc<C> {
  type Identifier = C::Identifier;
  type Message = C::Message;

  fn get_identifier(&self) -> Self::Identifier {
    (**self).get_identifier()
  }

  fn get_receiver(&self) -> broadcast::Receiver<ChannelEvent<Self::Message>> {
    (**self).get_receiver()
  }

  fn send_message(&self, content: String, nonce: String) -> Self::Message {
    (**self).send_message(content, nonce)
  }

  fn send_reply(&self, content: String, nonce: String, reply_to: Self::Identifier) -> Self::Message {
    (**self).send_reply(content, nonce, reply_to)
  }

  fn edit_message(&self, message: Self::Identifier, content: String) {
    (**self).edit_message(message, content)
  }

  fn delete_message(&self, message: Self::Identifier) {
    (**self).delete_message(message)
  }

  fn add_reaction(&self, message: Self::Identifier, emoji: Emoji) {
    (**self).add_reaction(message, emoji)
  }

  fn remove_reaction(&self, message: Self::Identifier, emoji: Emoji) {
    (**self).remove_reaction(message, emoji)
  }

  fn typing(&self) {
    (**self).typing()
  }
}
