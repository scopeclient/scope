pub mod app_state;
pub mod channel;

use std::{fs, path::PathBuf, sync::Arc};

use app_state::AppState;
use channel::ChannelView;
use components::theme::Theme;
use gpui::*;
use scope_backend_discord::{channel::DiscordChannel, client::DiscordClient, message::DiscordMessage, snowflake::Snowflake};

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

  let token = dotenv::var("DISCORD_TOKEN").expect("Must provide DISCORD_TOKEN in .env");
  let demo_channel_id = dotenv::var("DEMO_CHANNEL_ID").expect("Must provide DEMO_CHANNEL_ID in .env");

  let client = DiscordClient::new(token).await;

  let channel = DiscordChannel::new(
    client.clone(),
    Snowflake {
      content: demo_channel_id.parse().unwrap(),
    },
  )
  .await;

  App::new().with_assets(Assets { base: PathBuf::from("img") }).with_http_client(Arc::new(reqwest_client::ReqwestClient::new())).run(
    move |cx: &mut AppContext| {
      AppState::set_global(Arc::downgrade(&app_state), cx);

      if let Err(e) = init(app_state.clone(), cx) {
        log::error!("{}", e);
        return;
      }

      Theme::sync_system_appearance(cx);

      cx.open_window(WindowOptions::default(), |cx| ChannelView::<DiscordMessage>::create(cx, channel)).unwrap();
    },
  );
}
