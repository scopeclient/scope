use gpui::http_client::anyhow;
use gpui::{AssetSource, SharedString};

#[cfg(feature = "twemoji")]
use twemoji_assets::svg::SvgTwemojiAsset;

#[derive(rust_embed::RustEmbed)]
#[folder = "../../assets"]
pub(crate) struct Assets;

impl AssetSource for Assets {
  fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
    #[cfg(feature = "twemoji")]
    if path.starts_with("twemoji/") {
      let path = path.strip_prefix("twemoji/").unwrap();

      let data = SvgTwemojiAsset::from_emoji(path)
        .map(|f| f.data.0)
        .map(str::as_bytes)
        .map(std::borrow::Cow::Borrowed);

      return Ok(data);
    }

    Self::get(path).map(|f| Some(f.data)).ok_or_else(|| anyhow!("could not find asset at path \"{}\"", path))
  }

  fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
    Ok(Self::iter().filter_map(|p| if p.starts_with(path) { Some(p.into()) } else { None }).collect())
  }
}
