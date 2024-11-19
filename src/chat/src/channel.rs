use tokio::sync::broadcast;

use crate::message::Message;

pub trait Channel: Clone {
  type Message: Message;

  fn get_receiver(&self) -> broadcast::Receiver<Self::Message>;

  fn send_message(&self, content: String, nonce: String) -> Self::Message;
}

pub trait ChannelMetadata {
  fn get_name(&self) -> String;
  fn get_id(&self) -> String;
  fn get_icon(&self) -> Option<String>;
}