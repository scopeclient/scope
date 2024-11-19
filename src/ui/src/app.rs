use crate::channel::ChannelView;
use crate::sidebar::Sidebar;
use components::theme::ActiveTheme;
use gpui::prelude::FluentBuilder;
use gpui::{div, img, rgb, Context, Model, ParentElement, Render, Styled, View, ViewContext, VisualContext};
use scope_backend_discord::channel::DiscordChannelMetadata;
use scope_backend_discord::{channel::DiscordChannel, client::DiscordClient, message::DiscordMessage, snowflake::Snowflake};

pub struct App {
  channel: Model<Option<View<ChannelView<DiscordMessage>>>>,
  sidebar: Model<View<Sidebar<DiscordChannelMetadata>>>,
}

impl App {
  pub fn new(ctx: &mut ViewContext<'_, Self>) -> App {
    let token = dotenv::var("DISCORD_TOKEN").expect("Must provide DISCORD_TOKEN in .env");
    let demo_channel_id = dotenv::var("DEMO_CHANNEL_ID").expect("Must provide DEMO_CHANNEL_ID in .env");

    let mut context = ctx.to_async();
    let channel = ctx.new_model(|_| None);
    let async_channel = channel.clone();

    let sidebar_view = ctx.new_view(|cx| Sidebar::create(cx, None));
    let sidebar = ctx.new_model(|cx| sidebar_view);
    let async_sidebar = sidebar.clone();

    ctx
      .foreground_executor()
      .spawn(async move {
        let client = DiscordClient::new(token).await;

        let channel = DiscordChannel::new(
          client.clone(),
          Snowflake {
            content: demo_channel_id.parse().unwrap(),
          },
        )
        .await;

        let channel_view = context.new_view(|cx| ChannelView::<DiscordMessage>::create(cx, channel)).unwrap();

        let _ = async_channel.update(&mut context, |a, b| {
          *a = Some(channel_view);
          b.notify()
        });

        let sidebar_view = context.new_view(|cx| Sidebar::create(cx, Some(client))).unwrap();
        let _ = async_sidebar.update(&mut context, |a, b| {
          *a = sidebar_view;
          b.notify()
        });
      })
      .detach();

    App { channel, sidebar }
  }
}

impl Render for App {
  fn render(&mut self, cx: &mut ViewContext<Self>) -> impl gpui::IntoElement {
    let content = div()
      .w_full()
      .h_full()
      .flex()
      .gap_2()
      .child(self.sidebar.read(cx).clone())
      .when_some(self.channel.read(cx).clone(), |div, view| div.child(view));

    let title_bar = components::TitleBar::new()
      .child(div().flex().flex_row().text_color(rgb(0xFFFFFF)).gap_2().child(img("brand/scope-round-200.png").w_6().h_6()).child("Scope"));

    div().bg(cx.theme().background).w_full().h_full().flex().flex_col().child(title_bar).child(content)
  }
}
