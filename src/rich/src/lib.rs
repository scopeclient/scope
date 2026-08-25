//! Backend-agnostic rich message content: the data model (`model`), the
//! Discord-flavoured markdown parser (`markdown`), and the gpui renderer
//! (`view`). Backends convert their native message into a [`RichMessage`];
//! the UI renders it with [`RichContentView`].

pub mod cell;
pub mod emoji;
mod emoji_bundled;
pub mod markdown;
pub mod model;
pub mod view;

pub use cell::ContentCell;
pub use model::*;
pub use view::RichContentView;
