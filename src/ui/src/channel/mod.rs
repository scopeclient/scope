pub mod message;
pub mod message_list;

use components::input::{InputEvent, TextInput};
use gpui::{div, IntoElement, ParentElement, Pixels, Render, Styled, View, VisualContext};
use message::{message, MessageGroup};
use message_list::MessageListComponent;
use scope_chat::channel::Channel;

pub struct ChannelView<C: Channel + 'static> {
  list_view: View<MessageListComponent<C>>,
  message_input: View<TextInput>,
}

impl<C: Channel + 'static> ChannelView<C> {
  pub fn create(ctx: &mut gpui::ViewContext<'_, ChannelView<C>>, channel: C) -> Self {
    let mut channel_listener = channel.get_receiver();

    let list_view = ctx.new_view(|cx| MessageListComponent::create(cx, channel, Pixels(30.)));

    let async_model = list_view.clone();
    let mut async_ctx = ctx.to_async();

    ctx
      .foreground_executor()
      .spawn(async move {
        loop {
          let message = channel_listener.recv().await.unwrap();

          async_model
            .update(&mut async_ctx, |data, ctx| {
              // data.add_external_message(message);
              todo!();
              ctx.notify();
            })
            .unwrap();
        }
      })
      .detach();

    let message_input = ctx.new_view(|cx| {
      let mut input = components::input::TextInput::new(cx);

      input.set_size(components::Size::Large, cx);

      input
    });

    ctx
      .subscribe(&message_input, move |channel_view, text_input, input_event, ctx| {
        if let InputEvent::PressEnter = input_event {
          // let content = text_input.read(ctx).text().to_string();
          // if content.is_empty() {
          //   return;
          // }
          // let channel_sender = channel.clone();

          // text_input.update(ctx, |text_input, cx| {
          //   text_input.set_text("", cx);
          // });

          // let nonce = random_string::generate(20, random_string::charsets::ALPHANUMERIC);
          // let pending = channel.send_message(content, nonce);

          // channel_view.list_model.update(ctx, move |v, _| unimplemented!());
          // ctx.notify();
        }
      })
      .detach();

    ChannelView::<C> { list_view, message_input }
  }
}

impl<C: Channel + 'static> Render for ChannelView<C> {
  fn render(&mut self, cx: &mut gpui::ViewContext<Self>) -> impl gpui::IntoElement {
    div()
      .flex()
      .flex_col()
      .w_full()
      .h_full()
      .p_6()
      .child(div().w_full().h_full().flex().flex_col().child(self.list_view.clone()))
      .child(self.message_input.clone())
  }
}
