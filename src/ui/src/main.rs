pub mod actions;
pub mod app;
pub mod app_state;
pub mod channel;
pub mod menu;
pub mod sidebar;

use std::sync::Arc;

use app_state::AppState;
use components::theme::{hsl, Theme, ThemeColor, ThemeMode};
use gpui::*;
use http_client::anyhow;
use menu::app_menus;

#[derive(rust_embed::RustEmbed)]
#[folder = "../../assets"]
struct Assets;

impl AssetSource for Assets {
  fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
    Self::get(path).map(|f| Some(f.data)).ok_or_else(|| anyhow!("could not find asset at path \"{}\"", path))
  }

  fn list(&self, path: &str) -> Result<Vec<SharedString>> {
    Ok(Self::iter().filter_map(|p| if p.starts_with(path) { Some(p.into()) } else { None }).collect())
  }
}

fn init(_: Arc<AppState>, cx: &mut AppContext) -> Result<()> {
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

  App::new().with_assets(Assets).with_http_client(Arc::new(reqwest_client::ReqwestClient::new())).run(move |cx: &mut AppContext| {
    AppState::set_global(Arc::downgrade(&app_state), cx);

    if let Err(e) = init(app_state.clone(), cx) {
      log::error!("{}", e);
      return;
    }

    let mut theme = Theme::from(ThemeColor::dark());
    theme.mode = ThemeMode::Dark;
    theme.accent = hsl(335.0, 97.0, 61.0);
    theme.title_bar = hsl(335.0, 97.0, 61.0);
    theme.background = hsl(225.0, 12.0, 10.0);

    cx.set_global(theme);
    cx.refresh();

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

    cx.open_window(opts, |cx| cx.new_view(crate::app::App::new)).unwrap();
  });
}
