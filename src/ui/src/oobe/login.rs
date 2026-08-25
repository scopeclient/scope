use std::{
  io::{Read, Write},
  path::Path,
};

use components::{button::Button, IconName, StyledExt};
use gpui::{
  div, img, rgb, size, svg, AsyncAppContext, Bounds, ClickEvent, Context, Element, IntoElement, ParentElement, Pixels, Render, RenderOnce,
  SharedString, Styled, TitlebarOptions, View, VisualContext, WindowContext, WindowDecorations, WindowOptions,
};

use super::{input::OobeInput, titlebar::OobeTitleBar};

pub struct OobeTokenLogin {
  token_is_ready_to_send: bool,
  token: Option<String>,
  token_input: View<OobeInput>,
}

impl OobeTokenLogin {
  fn try_load_persistent_token() -> Option<String> {
    if let Some(data_dirs) = directories::ProjectDirs::from("com", "Scope Client", "Scope") {
      //TODO: Stabilize on a data model for long-term data storage.
      //      This PR sees this as a non-goal, however.

      let _ = std::fs::create_dir_all(data_dirs.data_local_dir());

      match std::fs::File::open(data_dirs.data_local_dir().join(Path::new("token"))) {
        Err(e) => {
          log::warn!("Failed to open the token file: {:?}", e);
          return None;
        }
        Ok(mut file) => {
          let mut token = String::new();

          if let Err(e) = file.read_to_string(&mut token) {
            log::warn!("Failed to read the token file: {:?}", e);
            return None;
          }

          return Some(token);
        }
      }
    } else {
      log::warn!("No data dir");
    }

    None
  }

  fn try_store_persistent_token(token: &String) {
    if let Some(data_dirs) = directories::ProjectDirs::from("com", "Scope Client", "Scope") {
      match std::fs::File::create(data_dirs.data_local_dir().join(Path::new("token"))) {
        Err(e) => {
          log::warn!("Failed to open the token file for write: {:?}", e);
          return;
        }

        Ok(mut file) => match file.write_all(token.as_bytes()) {
          Ok(_) => return,

          Err(e) => {
            log::warn!("Failed to write to the token file: {:?}", e);
            return;
          }
        },
      }
    }
  }

  async fn get_token_from_oobe(cx: &mut AsyncAppContext) -> Option<String> {
    let size = size(Pixels(450.0), Pixels(500.0));

    let window_options = cx
      .update(|cx| WindowOptions {
        window_decorations: Some(WindowDecorations::Client),
        window_min_size: Some(size),
        window_bounds: Some(gpui::WindowBounds::Windowed(Bounds::centered(None, size, cx))),
        titlebar: Some(TitlebarOptions {
          appears_transparent: true,
          title: Some(SharedString::new_static("scope")),
          ..Default::default()
        }),
        ..Default::default()
      })
      .unwrap();

    let window = cx.open_window(window_options, |cx| Self::build_root_view(cx)).unwrap();

    let (token_sender, receiver) = catty::oneshot();

    let mut token_sender = Some(token_sender);

    cx.update_window(*window, |win, cx| {
      cx.observe(&win.downcast::<OobeTokenLogin>().unwrap().model, move |model, cx| {
        let model = model.read(cx);

        let token_is_ready_to_send = model.token_is_ready_to_send;
        let token = model.token.clone();

        println!("Updates");

        cx.remove_window();

        if token_is_ready_to_send {
          token_sender.take().expect("Cannot double send").send(token.unwrap()).unwrap()
        }
      })
      .detach();
    })
    .unwrap();

    let token = receiver.await.ok();

    if let Some(ref token) = token {
      Self::try_store_persistent_token(token);
    }

    token
  }

  pub async fn get_token(cx: &mut AsyncAppContext, force_oobe: bool) -> Option<String> {
    if force_oobe {
      return Self::get_token_from_oobe(cx).await;
    }

    if let Ok(token_in_env) = dotenv::var("DISCORD_TOKEN") {
      Some(token_in_env)
    } else if let Some(token_in_persistent_storage) = Self::try_load_persistent_token() {
      Some(token_in_persistent_storage)
    } else {
      Self::get_token_from_oobe(cx).await
    }
  }

  fn build_root_view(cx: &mut WindowContext) -> gpui::View<Self> {
    let token_input = cx.new_view(|cx| OobeInput::create(cx, "Your Discord Token".to_owned(), true));

    cx.new_view(|_| Self {
      token_is_ready_to_send: false,
      token: None,
      token_input,
    })
  }
}

impl Render for OobeTokenLogin {
  fn render(&mut self, cx: &mut gpui::ViewContext<Self>) -> impl gpui::IntoElement {
    let title_bar = OobeTitleBar::new()
      .child(div().flex().flex_row().text_color(rgb(0x0F1013)).gap_2().child(img("brand/scope-round-200.png").w_6().h_6()).child("Scope"));

    div().child(img("brand/scope-login-bg.png").bg(rgb(0x0F1013)).absolute().top_0().left_0().w(Pixels(450.)).h(Pixels(500.))).child(
      div().absolute().top_0().left_0().w(Pixels(450.)).h(Pixels(500.)).flex().flex_col().child(title_bar).child(
        div()
          .flex()
          .flex_col()
          .justify_between()
          .w_full()
          .h_full()
          .pt(Pixels(12.))
          .px(Pixels(47.))
          .pb(Pixels(43.))
          .text_color(gpui::white())
          .child(
            div()
              .flex()
              .flex_col()
              .child(
                div()
                  .flex()
                  .flex_row()
                  .gap(Pixels(8.))
                  .child(svg().text_color(rgb(0xFC3B8C)).w(Pixels(16.)).h(Pixels(16.)).path("brand/reticle.svg"))
                  .child(div().text_color(rgb(0x65687A)).relative().top(Pixels(-5.)).m_0().font_bold().text_size(Pixels(16.)).child("SCOPE S1")),
              )
              .child(div().text_size(Pixels(36.)).text_color(rgb(0xE2E5ED)).font_extrabold().child("Login to Scope")),
          )
          .child(self.token_input.clone())
          .child(
            div().w_full().flex().flex_row().justify_end().child(Button::new("continue").label("Log In").icon(IconName::ArrowRight).on_click(
              cx.listener(|view, _, cx| {
                view.token_is_ready_to_send = true;
                view.token = Some(view.token_input.model.read(cx).input.model.read(cx).text().to_string());
                cx.notify();
              }),
            )),
          ),
      ),
    )
  }
}
