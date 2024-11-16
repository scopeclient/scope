pub mod channel;

use std::{fs, path::PathBuf, sync::Arc};

use channel::ChannelView;
use gpui::*;
use scope_backend_discord::{channel::DiscordChannel, client::DiscordClient, message::DiscordMessage, snowflake::Snowflake};
use scope_chat::channel::Channel;

struct HelloWorld {
    text: SharedString,
}
 
impl Render for HelloWorld {
    fn render(&mut self, _cx: &mut ViewContext<Self>) -> impl IntoElement {
        div()
            .flex()
            .bg(rgb(0x2e7d32))
            .size_full()
            .justify_center()
            .items_center()
            .text_xl()
            .text_color(rgb(0xffffff))
            .child(format!("Hello, {}!", &self.text))
    }
}

struct Assets {
    base: PathBuf,
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
        fs::read(self.base.join(path))
            .map(|data| Some(std::borrow::Cow::Owned(data)))
            .map_err(|e| e.into())
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        fs::read_dir(self.base.join(path))
            .map(|entries| {
                entries
                    .filter_map(|entry| {
                        entry
                            .ok()
                            .and_then(|entry| entry.file_name().into_string().ok())
                            .map(SharedString::from)
                    })
                    .collect()
            })
            .map_err(|e| e.into())
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let token = dotenv::var("DISCORD_TOKEN").unwrap();
    let demo_channel_id = dotenv::var("DEMO_CHANNEL_ID").unwrap();

    let mut client = DiscordClient::new(token);

    let mut channel = DiscordChannel::new(&mut client, Snowflake { content: demo_channel_id.parse().unwrap() }).await;

    App::new()
        .with_assets(Assets {
            base: PathBuf::from("img"),
        })
        .with_http_client(Arc::new(reqwest_client::ReqwestClient::new()))
        .run(|cx: &mut AppContext| {
            let window = cx.open_window(WindowOptions::default(), |cx| {
                ChannelView::<DiscordMessage>::create(cx, channel)
            })
            .unwrap();
        });
}
