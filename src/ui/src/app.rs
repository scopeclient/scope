use gpui::{AppContext as _, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div};
use gpui_component::ActiveTheme as _;
use scope_backend_discord::client::TokenKind;

use crate::{login::Login, shell::Shell, state::AppState};

/// Root view of the application window: shows the login screen until a
/// backend connection exists, then the main shell.
pub struct Scope {
  state: Entity<AppState>,
  login: Entity<Login>,
  shell: Option<Entity<Shell>>,
}

impl Scope {
  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let state = cx.new(|_| AppState::new());

    cx.observe(&state, |_, _, cx| cx.notify()).detach();

    let login = cx.new(|cx| Login::new(state.clone(), window, cx));

    // `SCOPE_DEMO=1` previews the UI with canned data; otherwise a token in the
    // environment (or `.env`) skips the login screen.
    if std::env::var("SCOPE_DEMO").is_ok_and(|v| v == "1") {
      state.update(cx, |state, cx| state.connect_demo(window, cx));
    } else if let Ok(token) = dotenvy::var("DISCORD_BOT_TOKEN") {
      state.update(cx, |state, cx| state.connect(token, TokenKind::Bot, false, cx));
    } else if let Ok(token) = dotenvy::var("DISCORD_TOKEN") {
      // `DISCORD_TOKEN_KIND=user` marks a user-account token; bots are the default.
      let kind = match dotenvy::var("DISCORD_TOKEN_KIND").map(|v| v.to_ascii_lowercase()) {
        Ok(v) if v == "user" => TokenKind::User,
        _ => TokenKind::Bot,
      };
      state.update(cx, |state, cx| state.connect(token, kind, false, cx));
    } else if let Some((token, kind)) = crate::auth::load() {
      // A token remembered from a previous login skips the login screen.
      state.update(cx, |state, cx| state.connect(token, kind, true, cx));
    }

    Scope { state, login, shell: None }
  }
}

impl Render for Scope {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let connected = self.state.read(cx).is_connected();

    let content = if connected {
      let shell = self.shell.get_or_insert_with(|| {
        let state = self.state.clone();
        cx.new(|cx| Shell::new(state, window, cx))
      });
      shell.clone().into_any_element()
    } else {
      self.login.clone().into_any_element()
    };

    div().bg(cx.theme().background).size_full().child(content)
  }
}
