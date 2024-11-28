use tokio::sync::broadcast;

use crate::{async_list::AsyncList, message::Message};
use crate::reaction::ReactionOperation;

pub trait Channel: AsyncList<Content = Self::Message> + Send + Sync + Clone {
  type Message: Message;

  fn get_message_receiver(&self) -> broadcast::Receiver<Self::Message>;
  fn get_reaction_receiver(&self) -> broadcast::Receiver<(String, ReactionOperation)>;

  fn send_message(&self, content: String, nonce: String) -> Self::Message;
}
