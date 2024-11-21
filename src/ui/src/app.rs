use components::theme::ActiveTheme;
use gpui::{div, img, rgb, Context, Model, ParentElement, Render, Styled, View, ViewContext, VisualContext};
use scope_backend_discord::{channel::DiscordChannel, client::DiscordClient, snowflake::Snowflake};

use crate::channel::ChannelView;

pub struct App {
  channel: Model<Option<View<ChannelView<DiscordChannel>>>>,
}

impl App {
  pub fn new(ctx: &mut ViewContext<'_, Self>) -> App {
    let token = dotenv::var("DISCORD_TOKEN").expect("Must provide DISCORD_TOKEN in .env");
    let demo_channel_id = dotenv::var("DEMO_CHANNEL_ID").expect("Must provide DEMO_CHANNEL_ID in .env");

    let mut context = ctx.to_async();

    let channel = ctx.new_model(|_| None);

    let async_channel = channel.clone();

    ctx
      .foreground_executor()
      .spawn(async move {
        println!("Trying to make a client...");

        let client = DiscordClient::new(token).await;

        println!("Made a client 👶");

        let channel = client.channel(Snowflake(demo_channel_id.parse().unwrap())).await;

        println!("Made a channel 👶");

        let view = context.new_view(|cx| ChannelView::<DiscordChannel>::create(cx, channel)).unwrap();

        async_channel
          .update(&mut context, |a, b| {
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
  fn render(&mut self, cx: &mut gpui::ViewContext<Self>) -> impl gpui::IntoElement {
    println!("Rendering App 📍");
    let mut content = div().w_full().h_full();

    if let Some(channel) = self.channel.read(cx).as_ref() {
      println!("We have a channel");
      content = content.child(channel.clone());
    }

    let title_bar = components::TitleBar::new()
      .child(div().flex().flex_row().text_color(rgb(0xFFFFFF)).gap_2().child(img("brand/scope-round-200.png").w_6().h_6()).child("Scope"));

    div().bg(cx.theme().background).w_full().h_full().flex().flex_col().child(title_bar).child(content)
  }
}
