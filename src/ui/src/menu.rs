use gpui::{Menu, MenuItem};

use crate::actions;

pub fn app_menus() -> Vec<Menu> {
  vec![
    Menu {
      name: "scope".into(),
      items: vec![MenuItem::action("quit", actions::Quit)],
    },
    Menu {
      name: "window".into(),
      items: vec![MenuItem::action("hide", actions::Hide)],
    },
  ]
}
