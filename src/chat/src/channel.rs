use tokio::sync::broadcast;

use crate::message::Message;

pub trait Channel {
  type Message: Message;

  fn get_receiver(&self) -> broadcast::Receiver<Self::Message>;
}