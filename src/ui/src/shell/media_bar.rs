//! Persistent "now playing" strip for the global media player.
//!
//! Sits between the channel bar and the chat area and spans everything right
//! of the main nav, so playback survives switching channels, tabs and guilds.
//! Hidden while no track is loaded.

use gpui::{
  App, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::h_flex;
use scope_media::{
  MediaPlayer, PlaybackStatus,
  element::{format_progress, progress_bar},
};

use crate::{shell::tabs::tooltip, state::AppState, theme::tokens};

pub const MEDIA_BAR_HEIGHT: f32 = 40.;

pub struct MediaBar {
  #[allow(dead_code)]
  state: Entity<AppState>,
}

impl MediaBar {
  pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
    cx.observe(&state, |_, _, cx| cx.notify()).detach();
    cx.observe(&MediaPlayer::state(cx), |_, _, cx| cx.notify()).detach();
    MediaBar { state }
  }
}

/// 24px circular play/pause control; `pause` shows the pause glyph.
fn play_pause_button(pause: bool) -> gpui::Stateful<gpui::Div> {
  let glyph = if pause {
    div().flex().flex_row().gap(px(2.5)).children([(), ()].map(|_| div().w(px(2.5)).h(px(9.)).rounded(px(1.25)).bg(gpui::white()))).into_any_element()
  } else {
    gpui::svg().path("icons/scope/rich-play.svg").size(px(10.)).text_color(gpui::white()).ml(px(1.)).into_any_element()
  };

  div()
    .id("media-toggle")
    .flex_shrink_0()
    .size(px(24.))
    .rounded_full()
    .bg(tokens::BRAND)
    .hover(|style| style.bg(tokens::BRAND_HOVER))
    .active(|style| style.bg(tokens::BRAND_ACTIVE))
    .flex()
    .items_center()
    .justify_center()
    .cursor_pointer()
    .tooltip(tooltip(if pause { "pause" } else { "play" }))
    .child(glyph)
    .on_click(|_, _, cx| MediaPlayer::toggle(cx))
}

/// Small icon button on the right side of the bar.
fn icon_button(
  id: &'static str,
  path: &'static str,
  size: f32,
  tip: &'static str,
  dimmed: bool,
  on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<gpui::Div> {
  // gpui's raw `svg` only paints with its own text color, so the hover tint
  // lives on the icon via `group_hover` rather than cascading from the div.
  div()
    .id(id)
    .group(id)
    .flex_shrink_0()
    .size(px(20.))
    .rounded(tokens::RADIUS_100)
    .flex()
    .items_center()
    .justify_center()
    .cursor_pointer()
    .hover(|style| style.bg(tokens::BG_FILL_TERTIARY))
    .tooltip(tooltip(tip))
    .child(
      gpui::svg()
        .path(path)
        .size(px(size))
        .flex_shrink_0()
        .text_color(if dimmed { tokens::TEXT_MUTED } else { tokens::ICON })
        .group_hover(id, |style| style.text_color(tokens::ICON_HOVER)),
    )
    .on_click(on_click)
}

impl Render for MediaBar {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let player = MediaPlayer::state(cx);
    let state = player.read(cx);

    let Some(track) = state.track.clone() else {
      return div().into_any_element();
    };

    let status = state.status.clone();
    let position = state.position;
    let duration = state.duration;
    let fraction = state.fraction().unwrap_or(0.);
    let muted = state.muted;

    let pause = matches!(status, PlaybackStatus::Playing | PlaybackStatus::Loading);
    let seek_enabled = duration.is_some() && matches!(status, PlaybackStatus::Playing | PlaybackStatus::Paused);

    // Title + subtitle; playback errors replace the subtitle.
    let subtitle: Option<(String, gpui::Hsla)> = match &status {
      PlaybackStatus::Error(message) => Some((message.clone(), tokens::TEXT_DANGER)),
      _ => track.subtitle.clone().map(|s| (s, tokens::TEXT_TERTIARY)),
    };

    let label = h_flex()
      .flex_shrink_0()
      .max_w(px(320.))
      .min_w_0()
      .gap(px(8.))
      .items_baseline()
      .child(div().min_w_0().truncate().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(tokens::TEXT).child(track.title.clone()))
      .children(subtitle.map(|(text, color)| div().min_w_0().truncate().text_size(tokens::TYPE_S).text_color(color).child(text)));

    let time = div().flex_shrink_0().text_size(tokens::TYPE_S).text_color(tokens::TEXT_SECONDARY).child(match status {
      PlaybackStatus::Loading => "loading…".to_owned(),
      _ => format_progress(position, duration),
    });

    h_flex()
      .w_full()
      .h(px(MEDIA_BAR_HEIGHT))
      .flex_shrink_0()
      .px(px(16.))
      .gap(px(12.))
      .items_center()
      .bg(tokens::BG_SURFACE_SECONDARY)
      .border_b_1()
      .border_color(tokens::BORDER)
      .child(play_pause_button(pause))
      .child(label)
      .child(progress_bar(fraction, MEDIA_BAR_HEIGHT, tokens::BG_FILL, tokens::BRAND, seek_enabled))
      .child(time)
      .child(icon_button(
        "media-mute",
        "icons/scope/channelnav-volume.svg",
        16.,
        if muted { "unmute" } else { "mute" },
        muted,
        |_, _, cx| MediaPlayer::toggle_mute(cx),
      ))
      .child(icon_button("media-stop", "icons/scope/close.svg", 10., "stop", false, |_, _, cx| {
        MediaPlayer::stop(cx)
      }))
      .into_any_element()
  }
}
