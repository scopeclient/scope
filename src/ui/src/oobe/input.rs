use components::input::TextInput;
use gpui::{div, rgb, Context, Model, ParentElement, Pixels, Render, Styled, View, VisualContext};

pub struct OobeInput {
  title: String,
  pub input: View<TextInput>,
}

impl OobeInput {
  pub fn create(ctx: &mut gpui::ViewContext<'_, Self>, title: String, secure: bool) -> Self {
    let input = ctx.new_view(|cx| {
      let mut input = TextInput::new(cx);

      if secure {
        input.set_masked(true, cx);
      }

      input
    });

    OobeInput { title, input }
  }
}

impl Render for OobeInput {
  fn render(&mut self, _: &mut gpui::ViewContext<Self>) -> impl gpui::IntoElement {
    div().flex().flex_col().gap(Pixels(4.)).text_color(rgb(0xA7ACBB)).child(self.title.clone()).child(self.input.clone())
  }
}
