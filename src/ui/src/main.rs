pub mod app;
pub mod app_state;
pub mod channel;

use std::{fs, path::PathBuf, sync::Arc};

use app_state::AppState;
use components::theme::Theme;
use gpui::*;

struct Assets {
  base: PathBuf,
}

impl AssetSource for Assets {
  fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
    fs::read(self.base.join(path)).map(|data| Some(std::borrow::Cow::Owned(data))).map_err(|e| e.into())
  }

  fn list(&self, path: &str) -> Result<Vec<SharedString>> {
    fs::read_dir(self.base.join(path))
      .map(|entries| entries.filter_map(|entry| entry.ok().and_then(|entry| entry.file_name().into_string().ok()).map(SharedString::from)).collect())
      .map_err(|e| e.into())
  }
}

actions!(main_menu, [Quit]);

fn init(_: Arc<AppState>, cx: &mut AppContext) -> Result<()> {
  components::init(cx);

  cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);

  Ok(())
}

#[tokio::main]
async fn main() {
  env_logger::init();

  let app_state = Arc::new(AppState {});

  App::new()
    .with_assets(Assets {
      base: PathBuf::from("assets"),
    })
    .with_http_client(Arc::new(reqwest_client::ReqwestClient::new()))
    .run(move |cx: &mut AppContext| {
      AppState::set_global(Arc::downgrade(&app_state), cx);

      if let Err(e) = init(app_state.clone(), cx) {
        log::error!("{}", e);
        return;
      }

      Theme::sync_system_appearance(cx);

      let opts = WindowOptions {
        window_decorations: Some(WindowDecorations::Client),
        titlebar: Some(TitlebarOptions {
          appears_transparent: true,
          title: Some(SharedString::new_static("scope")),
          ..Default::default()
        }),
        ..Default::default()
      };

      cx.open_window(opts, |cx| cx.new_view(|cx| crate::app::App::new(cx))).unwrap();
    });
}
