use gpui::{ListState, Pixels};
use scope_backend_discord::{message::{author::{DiscordMessageAuthor, DisplayName}, content::DiscordMessageContent, DiscordMessage}, snowflake::Snowflake};
use scope_chat::message::Message;

pub struct MessageList<M: Message> {
  pub (super) messages: Vec<M>,
}

impl<M: Message> MessageList<M> {
  pub fn new() -> MessageList<M> {
    Self {
      messages: Vec::default(),
    }
  }
}

// impl MessageList<DiscordMessage> {
//   pub fn new() -> MessageList<DiscordMessage> {
//     Self {
//       messages: vec![
//         DiscordMessage {
//           author: DiscordMessageAuthor {
//             display_name: DisplayName("Rose".to_owned()),
//             icon: "https://cdn.discordapp.com/avatars/519673297693048832/b4d62e9ea69dc07c7bb6298af490ebdf.png".to_owned(),
//           },
//           content: DiscordMessageContent {
//             content: "Demo\nContent".to_owned(),
//           },
//           id: Snowflake {
//             content: 512,
//           }
//         }
//       ],
//     }
//   }
// }