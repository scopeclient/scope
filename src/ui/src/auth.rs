//! Stored login: remembers the Discord token between launches so
//! `DISCORD_TOKEN` is optional. Plain file storage (0600 on unix) — the gpui
//! keychain API is a (url, username, password) tuple and cannot represent
//! multiple accounts, so it is deliberately not used here.

use std::{fs, path::PathBuf};

use scope_backend_discord::client::TokenKind;

/// `~/Library/Application Support/scope` / `$XDG_CONFIG_HOME/scope` / `%APPDATA%\scope`.
fn config_dir() -> Option<PathBuf> {
  if cfg!(target_os = "macos") {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support/scope"))
  } else if cfg!(target_os = "windows") {
    std::env::var_os("APPDATA").map(|base| PathBuf::from(base).join("scope"))
  } else {
    std::env::var_os("XDG_CONFIG_HOME")
      .map(PathBuf::from)
      .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
      .map(|base| base.join("scope"))
  }
}

fn token_file() -> Option<PathBuf> {
  config_dir().map(|dir| dir.join("auth.token"))
}

/// The stored token, if any. First line is the kind (`bot`/`user`), the rest is the token.
pub fn load() -> Option<(String, TokenKind)> {
  let contents = fs::read_to_string(token_file()?).ok()?;
  let (kind, token) = contents.split_once('\n')?;
  let token = token.trim().to_string();

  if token.is_empty() {
    return None;
  }

  let kind = match kind.trim() {
    "user" => TokenKind::User,
    _ => TokenKind::Bot,
  };

  Some((token, kind))
}

pub fn save(token: &str, kind: TokenKind) {
  let Some(path) = token_file() else { return };

  if let Some(dir) = path.parent()
    && let Err(error) = fs::create_dir_all(dir)
  {
    log::warn!("could not create config dir {dir:?}: {error}");
    return;
  }

  let kind = match kind {
    TokenKind::Bot => "bot",
    TokenKind::User => "user",
  };

  if let Err(error) = fs::write(&path, format!("{kind}\n{token}\n")) {
    log::warn!("could not store the login token: {error}");
    return;
  }

  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt as _;
    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
  }
}

/// Forget the stored token (e.g. after Discord rejects it).
pub fn forget() {
  if let Some(path) = token_file()
    && let Err(error) = fs::remove_file(&path)
    && error.kind() != std::io::ErrorKind::NotFound
  {
    log::warn!("could not remove the stored token: {error}");
  }
}
