use tokio::sync::broadcast;

use crate::{async_list::AsyncList, message::Message};

pub trait Channel: AsyncList<Content = Self::Message> + Send + Sync {
  type Message: Message;

  fn get_receiver(&self) -> broadcast::Receiver<Self::Message>;

  fn send_message(&self, content: String, nonce: String) -> Self::Message;
}
