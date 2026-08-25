pub mod actions;
pub mod app;
pub mod auth;
pub mod backend;
pub mod channel;
pub mod icons;
pub mod login;
pub mod menu;
pub mod shell;
pub mod state;
pub mod theme;

use std::{borrow::Cow, sync::Arc};

use gpui::{
  App, AppContext as _, Application, AssetSource, Bounds, KeyBinding, Result, SharedString, TitlebarOptions, WindowBounds, WindowDecorations,
  WindowOptions, point, px, size,
};
use gpui_component::Root;
use menu::app_menus;

/// Files under `/assets` (brand images, Scope-specific icons, fonts).
#[derive(rust_embed::RustEmbed)]
#[folder = "../../assets"]
struct ScopeAssets;

/// Asset source that serves Scope's own assets first and falls back to the
/// icon set bundled with gpui-component.
struct Assets;

impl AssetSource for Assets {
  fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
    if let Some(file) = ScopeAssets::get(path) {
      return Ok(Some(file.data));
    }

    gpui_component_assets::Assets.load(path)
  }

  fn list(&self, path: &str) -> Result<Vec<SharedString>> {
    let mut paths: Vec<SharedString> = ScopeAssets::iter().filter(|p| p.starts_with(path)).map(|p| p.to_string().into()).collect();
    paths.extend(gpui_component_assets::Assets.list(path)?);
    Ok(paths)
  }
}

fn init(cx: &mut App) {
  gpui_component::init(cx);
  scope_media::init(cx);
  theme::init(cx);

  if cfg!(target_os = "macos") {
    cx.bind_keys([
      KeyBinding::new("cmd-q", actions::Quit, None),
      KeyBinding::new("cmd-h", actions::Hide, None),
    ]);
  } else {
    cx.bind_keys([
      KeyBinding::new("ctrl-q", actions::Quit, None),
      KeyBinding::new("ctrl-h", actions::Hide, None),
    ]);
  }

  cx.set_menus(app_menus());

  cx.on_action(|_: &actions::Quit, cx| cx.quit());
  cx.on_action(|_: &actions::Hide, cx| cx.hide());
}

#[tokio::main]
async fn main() {
  // Warnings matter (failed sends, dropped events); show them by default.
  env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

  Application::new().with_assets(Assets).with_http_client(Arc::new(reqwest_client::ReqwestClient::new())).run(|cx| {
    init(cx);

    let opts = WindowOptions {
      window_decorations: Some(WindowDecorations::Client),
      window_min_size: Some(size(px(800.), px(600.))),
      window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1453.), px(1024.)), cx))),
      titlebar: Some(TitlebarOptions {
        appears_transparent: true,
        title: Some(SharedString::new_static("Scope")),
        traffic_light_position: Some(point(px(12.), px(14.))),
      }),
      ..Default::default()
    };

    cx.open_window(opts, |window, cx| {
      let view = cx.new(|cx| app::Scope::new(window, cx));
      cx.new(|cx| Root::new(view, window, cx))
    })
    .expect("failed to open the main window");

    cx.activate(true);
  });
}
