pub mod message;
pub mod message_list;

use components::input::{InputEvent, TextInput};
use gpui::{div, list, Context, ListState, Model, ParentElement, Render, Styled, View, VisualContext};
use message_list::MessageList;
use scope_chat::{channel::Channel, message::Message};

pub struct ChannelView<M: Message + 'static> {
  list_state: ListState,
  list_model: Model<MessageList<M>>,
  message_input: View<TextInput>,
}

impl<M: Message + 'static> ChannelView<M> {
  pub fn create(ctx: &mut gpui::ViewContext<'_, ChannelView<M>>, channel: impl Channel<Message = M> + 'static) -> Self {
    let state_model = ctx.new_model(|_cx| MessageList::<M>::new());

    let async_model = state_model.clone();
    let mut async_ctx = ctx.to_async();
    let mut channel_listener = channel.get_receiver();

    ctx
      .foreground_executor()
      .spawn(async move {
        loop {
          let message = channel_listener.recv().await.unwrap();

          async_model
            .update(&mut async_ctx, |data, ctx| {
              data.add_external_message(message);
              ctx.notify();
            })
            .unwrap();
        }
      })
      .detach();

    ctx
      .observe(&state_model, |this: &mut ChannelView<M>, model, cx| {
        this.list_state = model.read(cx).create_list_state();
        cx.notify();
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
          let content = text_input.read(ctx).text().to_string();
          if content.is_empty() {
            return;
          }
          let channel_sender = channel.clone();

          text_input.update(ctx, |text_input, cx| {
            text_input.set_text("", cx);
          });

          let nonce = random_string::generate(20, random_string::charsets::ALPHANUMERIC);
          let pending = channel.send_message(content, nonce);

          channel_view.list_model.update(ctx, move |v, _| {
            v.add_pending_message(pending);
          });
          channel_view.list_state = channel_view.list_model.read(ctx).create_list_state();
          ctx.notify();
        }
      })
      .detach();

    ChannelView::<M> {
      list_state: state_model.read(ctx).create_list_state(),
      list_model: state_model,
      message_input,
    }
  }
}

impl<M: Message + 'static> Render for ChannelView<M> {
  fn render(&mut self, _: &mut gpui::ViewContext<Self>) -> impl gpui::IntoElement {
    div().flex().flex_col().w_full().h_full().p_6().child(list(self.list_state.clone()).w_full().h_full()).child(self.message_input.clone())
  }
}
