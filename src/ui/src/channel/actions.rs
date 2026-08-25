//! Things a user can do to a message from the chat column. The list/rows
//! emit these; `ChannelView` decides what they mean for its channel.

use std::rc::Rc;

use gpui::{App, Window};
use scope_rich::Emoji;

#[derive(Clone)]
pub enum MessageAction<M> {
  /// Toggle a reaction (add if we haven't reacted, remove if we have).
  React { message: M, emoji: Emoji },
  Reply(M),
  Edit(M),
  Delete(M),
  CopyText(M),
}

pub type MessageActions<M> = Rc<dyn Fn(MessageAction<M>, &mut Window, &mut App)>;
