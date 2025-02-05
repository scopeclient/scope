pub mod actions;
pub mod app;
pub mod app_state;
mod assets;
pub mod channel;
pub mod menu;

use app_state::AppState;
use components::theme::{hsl, Theme, ThemeColor, ThemeMode};
use gpui::*;
use menu::app_menus;
use std::sync::Arc;

fn init(_: Arc<AppState>, cx: &mut App) -> Result<()> {
  components::init(cx);

  if cfg!(target_os = "macos") {
    cx.bind_keys(vec![KeyBinding::new("cmd-q", actions::Quit, None)]);
    cx.bind_keys(vec![KeyBinding::new("cmd-h", actions::Hide, None)]);
  } else {
    cx.bind_keys(vec![KeyBinding::new("ctrl-h", actions::Hide, None)]);
  }

  cx.set_menus(app_menus());

  cx.on_action(|_: &actions::Quit, cx| cx.quit());
  cx.on_action(|_: &actions::Hide, cx| cx.hide());

  Ok(())
}

#[tokio::main]
async fn main() {
  env_logger::init();

  let app_state = Arc::new(AppState {});

  Application::new().with_assets(assets::Assets).with_http_client(Arc::new(reqwest_client::ReqwestClient::new())).run(move |app: &mut App| {
    AppState::set_global(Arc::downgrade(&app_state), app);

    if let Err(e) = init(app_state.clone(), app) {
      log::error!("{}", e);
      return;
    }

    let mut theme = Theme::from(ThemeColor::dark());
    theme.mode = ThemeMode::Dark;
    theme.accent = hsl(335.0, 97.0, 61.0);
    theme.title_bar = hsl(335.0, 97.0, 61.0);
    theme.background = hsl(225.0, 12.0, 10.0);

    app.set_global(theme);
    app.refresh_windows();

    let opts = WindowOptions {
      window_decorations: Some(WindowDecorations::Client),
      window_min_size: Some(size(Pixels(800.0), Pixels(600.0))),
      titlebar: Some(TitlebarOptions {
        appears_transparent: true,
        title: Some(SharedString::new_static("scope")),
        ..Default::default()
      }),
      ..Default::default()
    };

    app.open_window(opts, |window: &mut Window, cx| cx.new(|cx| app::App::new(window, cx))).unwrap();
  });
}
