pub mod message;
pub mod message_list;

use std::sync::Arc;

use components::input::{InputEvent, TextInput};
use gpui::{div, AppContext, Entity, ParentElement, Pixels, Render, Styled};
use message_list::MessageListComponent;
use scope_chat::channel::Channel;

pub struct ChannelView<C: Channel + 'static> {
  list_view: Entity<MessageListComponent<Arc<C>>>,
  message_input: Entity<TextInput>,
}

impl<C: Channel + 'static> ChannelView<C> {
  pub fn create(window: &mut gpui::Window, ctx: &mut gpui::Context<'_, ChannelView<C>>, channel: Arc<C>) -> Self
  where
    <C as Channel>::Identifier: Send,
  {
    let channel_message_listener = channel.get_message_receiver();
    let channel_reaction_listener = channel.get_reaction_receiver();

    let c2 = channel.clone();

    let list_view = ctx.new(|cx| MessageListComponent::create(cx, channel, Pixels(30.)));

    let async_model = list_view.clone();
    let mut async_ctx = ctx.to_async();

    ctx
      .foreground_executor()
      .spawn(async move {
        loop {
          let (sender, receiver) = catty::oneshot();

          let mut l = channel_message_listener.resubscribe();

          tokio::spawn(async move {
            match sender.send(l.recv().await) {
              Ok(_) => {}
              Err(_e) => log::error!("Failed to send message data!"),
            };
          });

          let message = receiver.await.unwrap().unwrap();
          async_model
            .update(&mut async_ctx, |data, ctx| {
              data.append_message(ctx, message);
              ctx.notify();
            })
            .unwrap();
        }
      })
      .detach();

    let async_model = list_view.clone();
    let mut async_ctx = ctx.to_async();
    ctx
      .foreground_executor()
      .spawn(async move {
        let mut l = channel_reaction_listener.resubscribe();
        loop {
          let reaction = l.recv().await.unwrap();
          async_model
            .update(&mut async_ctx, |data, cx| {
              data.update_reaction(cx, reaction);
            })
            .unwrap();
        }
      })
      .detach();

    let message_input = ctx.new(|cx| {
      let mut input = TextInput::new(window, cx);

      input.set_size(components::Size::Large, window, cx);

      input
    });

    let async_model = list_view.clone();

    let mut async_window = window.to_async(ctx);
    ctx
      .subscribe(&message_input, move |_, text_input, input_event, ctx| {
        if let InputEvent::PressEnter = input_event {
          let content = text_input.read(ctx).text().to_string();
          if content.is_empty() {
            return;
          }

          text_input.update(ctx, |text_input, cx| {
            async_window.update(|window, _| text_input.set_text("", window, cx)).unwrap()
          });

          let nonce = random_string::generate(20, random_string::charsets::ALPHANUMERIC);
          let pending = c2.send_message(content, nonce);

          let mut async_ctx = ctx.to_async();

          let async_model = async_model.clone();

          ctx
            .foreground_executor()
            .spawn(async move {
              async_model
                .update(&mut async_ctx, |data, ctx| {
                  data.append_message(ctx, pending);
                  ctx.notify();
                })
                .unwrap();
            })
            .detach();

          ctx.notify();
        }
      })
      .detach();

    ChannelView::<C> { list_view, message_input }
  }
}

impl<C: Channel + 'static> Render for ChannelView<C> {
  fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
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
