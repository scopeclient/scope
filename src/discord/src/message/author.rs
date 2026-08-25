use std::sync::Arc;

use gpui::{App, IntoElement, ObjectFit, ParentElement, RenderOnce, SharedString, Styled, StyledImage, Window, div, img};
use scope_chat::message::{IconRenderConfig, MessageAuthor};
use url::Url;

use crate::{client::DiscordClient, snowflake::Snowflake};

#[derive(Clone)]
pub enum DiscordMessageAuthorData {
  User(Arc<serenity::model::user::User>),
  NonMemberAuthor(Arc<serenity::model::channel::Message>),
  Member(Arc<serenity::model::guild::Member>),
}

#[derive(Clone)]
pub struct DiscordMessageAuthor {
  pub data: DiscordMessageAuthorData,

  pub client: Arc<DiscordClient>,
}

impl PartialEq for DiscordMessageAuthor {
  fn eq(&self, other: &Self) -> bool {
    match (&self.data, &other.data) {
      (DiscordMessageAuthorData::Member(left), DiscordMessageAuthorData::Member(right)) => {
        left.guild_id == right.guild_id && left.user.id == right.user.id
      }
      (DiscordMessageAuthorData::User(left), DiscordMessageAuthorData::User(right)) => left.id == right.id,
      _ => false,
    }
  }
}
impl Eq for DiscordMessageAuthor {}

impl MessageAuthor for DiscordMessageAuthor {
  type Identifier = Snowflake;
  type DisplayName = DisplayName;
  type Icon = DisplayIcon;

  fn get_display_name(&self) -> Self::DisplayName {
    match &self.data {
      DiscordMessageAuthorData::Member(member) => DisplayName(member.display_name().to_owned().into()),
      DiscordMessageAuthorData::User(user) => DisplayName(user.display_name().to_owned().into()),
      DiscordMessageAuthorData::NonMemberAuthor(message) => DisplayName(message.author.display_name().to_owned().into()),
    }
  }

  fn get_icon(&self, config: IconRenderConfig) -> Self::Icon {
    match &self.data {
      DiscordMessageAuthorData::Member(member) => DisplayIcon(
        member.avatar_url().or(member.user.avatar_url()).unwrap_or(member.user.default_avatar_url()),
        config,
      ),
      DiscordMessageAuthorData::User(user) => DisplayIcon(user.avatar_url().unwrap_or(user.default_avatar_url()), config),
      DiscordMessageAuthorData::NonMemberAuthor(message) => {
        DisplayIcon(message.author.avatar_url().unwrap_or(message.author.default_avatar_url()), config)
      }
    }
  }

  fn get_identifier(&self) -> Self::Identifier {
    match &self.data {
      DiscordMessageAuthorData::Member(member) => member.user.id.into(),
      DiscordMessageAuthorData::User(user) => user.id.into(),
      DiscordMessageAuthorData::NonMemberAuthor(message) => message.author.id.into(),
    }
  }
}

/// The author's name. Inherits font size, weight and colour from the row that
/// places it so the chat UI owns the typography.
#[derive(Clone, IntoElement, Debug)]
pub struct DisplayName(pub SharedString);

impl RenderOnce for DisplayName {
  fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
    div().truncate().child(self.0)
  }
}

/// The author's avatar, filling whatever frame it is placed in. The CDN
/// `size` query parameter is set from the render config.
#[derive(Clone, IntoElement, Debug)]
pub struct DisplayIcon(pub String, pub IconRenderConfig);

impl RenderOnce for DisplayIcon {
  fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
    let mut url = Url::parse(&self.0).unwrap();
    let mut query_params = querystring::querify(url.query().unwrap_or(""));

    let mut key_found = false;
    let size = self.1.size().to_string();

    for (key, value) in query_params.iter_mut() {
      if key == &"size" {
        *value = &size;
        key_found = true;
      }
    }

    if !key_found {
      query_params.push(("size", &size));
    }

    url.set_query(Some(&querystring::stringify(query_params)));

    img(url.to_string()).size_full().rounded_full().object_fit(ObjectFit::Cover)
  }
}
