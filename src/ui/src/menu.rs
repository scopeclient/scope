use gpui::{Menu, MenuItem};

use crate::actions;

pub fn app_menus() -> Vec<Menu> {
  vec![
    Menu {
      name: "Scope".into(),
      items: vec![MenuItem::action("Quit", actions::Quit)],
    },
    Menu {
      name: "Window".into(),
      items: vec![MenuItem::action("Hide", actions::Hide)],
    },
  ]
}
