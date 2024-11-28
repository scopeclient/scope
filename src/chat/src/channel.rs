use tokio::sync::broadcast;

use crate::reaction::ReactionEvent;
use crate::{async_list::AsyncList, message::Message};

pub trait Channel: AsyncList<Content = Self::Message> + Send + Sync + Clone {
  type Message: Message;

  fn get_message_receiver(&self) -> broadcast::Receiver<Self::Message>;
  fn get_reaction_receiver(&self) -> broadcast::Receiver<ReactionEvent>;

  fn send_message(&self, content: String, nonce: String) -> Self::Message;
}
