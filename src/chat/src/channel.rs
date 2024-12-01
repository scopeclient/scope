use std::{fmt::Debug, sync::Arc};

use tokio::sync::broadcast;

use crate::reaction::ReactionEvent;
use crate::{async_list::AsyncList, message::Message};

pub trait Channel: AsyncList<Content = Self::Message> + Send + Sync + Clone {
  type Message: Message<Identifier = Self::Identifier>;
  type Identifier: Sized + Copy + Clone + Debug + Eq + PartialEq;

  fn get_message_receiver(&self) -> broadcast::Receiver<Self::Message>;
  fn get_reaction_receiver(&self) -> broadcast::Receiver<ReactionEvent>;

  fn send_message(&self, content: String, nonce: String) -> Self::Message;

  fn get_identifier(&self) -> Self::Identifier;
}

impl<C: Channel> Channel for Arc<C> {
  type Identifier = C::Identifier;
  type Message = C::Message;

  fn get_identifier(&self) -> Self::Identifier {
    (**self).get_identifier()
  }

  fn get_receiver(&self) -> broadcast::Receiver<Self::Message> {
    (**self).get_receiver()
  }

  fn send_message(&self, content: String, nonce: String) -> Self::Message {
    (**self).send_message(content, nonce)
  }
}
