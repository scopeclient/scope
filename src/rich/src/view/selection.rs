//! Message copy: click a message to select it, then Cmd/Ctrl+C (or Cmd/Ctrl+A
//! then copy) to put its text on the clipboard.
//!
//! TODO(selection): character-level drag selection. This is message-granular
//! for now — reliable and enough to make messages copyable.

use gpui::{App, Global, KeyBinding, actions};

pub const MESSAGE_CONTEXT: &str = "Message";

actions!(scope_message, [CopyMessage, SelectMessage]);

/// Bind copy/select-all in the message key context, once per app.
pub fn install_keymap(cx: &mut App) {
  if cx.has_global::<KeyMapMarker>() {
    return;
  }
  cx.set_global(KeyMapMarker);

  cx.bind_keys([
    KeyBinding::new("cmd-c", CopyMessage, Some(MESSAGE_CONTEXT)),
    KeyBinding::new("ctrl-c", CopyMessage, Some(MESSAGE_CONTEXT)),
    KeyBinding::new("cmd-a", SelectMessage, Some(MESSAGE_CONTEXT)),
    KeyBinding::new("ctrl-a", SelectMessage, Some(MESSAGE_CONTEXT)),
  ]);
}

// Distinct marker so we don't clash with other one-shot keymap globals.
struct KeyMapMarker;
impl Global for KeyMapMarker {}
