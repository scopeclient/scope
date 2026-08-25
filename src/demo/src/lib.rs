//! An offline backend with sample content that behaves like a live server:
//! history, sends, and a background task that keeps posting messages,
//! flipping presence and bumping unread counts. Enabled with `SCOPE_DEMO=1`.

pub mod channel;
pub mod client;
pub mod data;
pub mod message;

pub use channel::DemoChannel;
pub use client::DemoClient;
pub use message::{DemoAuthor, DemoMessage};
