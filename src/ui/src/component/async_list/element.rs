use std::future::Future;

use gpui::{div, rgb, AnyElement, Context, Model, ParentElement, Render, Styled, ViewContext};
use scope_chat::async_list::{AsyncListItem, AsyncListResult};

pub enum AsyncListComponentElement<E: AsyncListItem> {
  Waiting,
  Resolved(E),
  None,
}

pub struct AsyncListComponentElementView<E: AsyncListItem + 'static> {
  pub element: Model<AsyncListComponentElement<E>>,
  renderer: Box<dyn Fn(&E) -> AnyElement>,
}

impl<E: AsyncListItem> AsyncListComponentElementView<E> {
  pub fn new(
    ctx: &mut ViewContext<'_, Self>,
    renderer: impl (Fn(&E) -> AnyElement) + 'static,
    future: impl Future<Output = Option<AsyncListResult<E>>> + 'static,
  ) -> AsyncListComponentElementView<E> {
    let model = ctx.new_model(|_| AsyncListComponentElement::Waiting);

    let mut async_ctx = ctx.to_async();

    let model_handle = model.clone();

    ctx
      .foreground_executor()
      .spawn(async move {
        let result = future.await;

        async_ctx
          .update_model(&model_handle, |v, cx| {
            if let Some(result) = result {
              *v = AsyncListComponentElement::Resolved(result.content);
            } else {
              *v = AsyncListComponentElement::None;
            }
            cx.notify();
          })
          .unwrap();
      })
      .detach();

    AsyncListComponentElementView {
      element: model,
      renderer: Box::new(renderer),
    }
  }
}

impl<E: AsyncListItem> Render for AsyncListComponentElementView<E> {
  fn render(&mut self, cx: &mut ViewContext<Self>) -> impl gpui::IntoElement {
    match self.element.read(cx) {
      AsyncListComponentElement::Waiting => div().w_full().h_8().flex().items_center().justify_center().text_color(rgb(0xFFFFFF)).child("Waiting..."),
      AsyncListComponentElement::None => div().w_full().h_8().flex().items_center().justify_center().text_color(rgb(0xFFFFFF)).child("None!"),
      AsyncListComponentElement::Resolved(v) => div().w_full().h_full().child((self.renderer)(v)),
    }
  }
}
