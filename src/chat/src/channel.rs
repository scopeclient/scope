use crate::message::Message;

pub trait Channel {
  type Message: Message;

  fn add_message_listener(&mut self, func: fn (Self::Message) -> ());
}