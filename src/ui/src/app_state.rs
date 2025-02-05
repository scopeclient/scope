use std::sync::Weak;

use gpui::{App, Global};

pub struct AppState {}

struct GlobalAppState();

impl Global for GlobalAppState {}

impl AppState {
  pub fn set_global(_app_state: Weak<AppState>, cx: &mut App) {
    cx.set_global(GlobalAppState());
  }
}
