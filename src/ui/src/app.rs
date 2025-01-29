use components::theme::ActiveTheme;
use gpui::{div, img, rgb, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window};
use scope_backend_discord::{channel::DiscordChannel, client::DiscordClient, snowflake::Snowflake};

use crate::channel::ChannelView;

pub struct App {
  channel: Entity<Option<Entity<ChannelView<DiscordChannel>>>>,
}

impl App {
  pub fn new(window: &mut Window, cx: &mut gpui::App) -> App {
    let token = dotenv::var("DISCORD_TOKEN").expect("Must provide DISCORD_TOKEN in .env");
    let demo_channel_id = dotenv::var("DEMO_CHANNEL_ID").expect("Must provide DEMO_CHANNEL_ID in .env");

    let channel = cx.new(|_| None);

    let async_channel = channel.clone();
    window
      .spawn(cx, |mut cx| async move {
        let client = DiscordClient::new(token).await;
        let channel = client.channel(Snowflake(demo_channel_id.parse().unwrap())).await;
        let view = cx.update(|window, cx| cx.new(|cx| ChannelView::<DiscordChannel>::create(window, cx, channel))).unwrap();

        async_channel
          .update(&mut cx, |a, b| {
            *a = Some(view);
            b.notify()
          })
          .unwrap();
      })
      .detach();

    App { channel }
  }
}

impl Render for App {
  fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
    let mut content = div().w_full().h_full();

    if let Some(channel) = self.channel.read(cx).as_ref() {
      content = content.child(channel.clone());
    }

    let title_bar = components::TitleBar::new()
      .child(div().flex().flex_row().text_color(rgb(0xFFFFFF)).gap_2().child(img("brand/scope-round-200.png").w_6().h_6()).child("Scope"));

    div()
      .bg(cx.theme().background)
      .w_full()
      .h_full()
      .flex()
      .flex_col()
      .child(title_bar)
      .child(content)
  }
}
