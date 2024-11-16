use tokio::sync::broadcast;

use crate::message::Message;

pub trait Channel: Clone {
  type Message: Message;

  fn get_receiver(&self) -> broadcast::Receiver<Self::Message>;

  fn send_message(&self, content: String, nonce: String) -> impl std::future::Future<Output = ()>;
}