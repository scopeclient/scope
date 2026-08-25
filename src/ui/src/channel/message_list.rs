//! Bottom-anchored, lazily paged message list.
//!
//! The backend is paged one message at a time into `cache`; every change to
//! the cache is regrouped into `rows` (message groups plus the chrome between
//! them: date separators, the "new messages" divider, the start-of-history
//! note) and a fresh `ListState` is built with the scroll position carried
//! over in terms of rendered rows.

use std::{cell::Cell, rc::Rc, sync::Arc};

use chrono::{DateTime, Local, NaiveDate, Utc};
use gpui::{
  ClickEvent, Context, Entity, FontWeight, IntoElement, ListAlignment, ListOffset, ListState, ParentElement, Pixels, Render, SharedString, Styled,
  Window, div, list, prelude::*, px, white,
};
use scope_chat::{
  async_list::{AsyncListIndex, AsyncListItem},
  channel::Channel,
  message::{Message, MessageAuthor},
};
use tokio::sync::RwLock;

use super::{
  actions::MessageActions,
  message::{MessageGroup, loading_row, message_group},
};
use crate::theme::tokens;

/// Horizontal insets of the separator rules; match the message row padding
/// (avatar at x=17, right edge at 16) without depending on `message.rs`.
const ROW_PAD_LEFT: f32 = 17.;
const ROW_PAD_RIGHT: f32 = 16.;
const SEPARATOR_HEIGHT: f32 = 28.;
const SEPARATOR_PAD_Y: f32 = 8.;
const NEW_DIVIDER_HEIGHT: f32 = 16.;
const START_GAP: f32 = 24.;
const PILL_INSET: f32 = 12.;

/// What changed in the cache since the last rebuild, in cache items: `shift`
/// were inserted at the top, `new_items` appended at the bottom.
#[derive(Clone, Copy, Default)]
struct ListStateDirtyState {
  pub new_items: usize,
  pub shift: usize,
}

#[derive(Clone, Copy, Default)]
struct BoundFlags {
  pub before: bool,
  pub after: bool,
}

#[derive(Debug)]
pub enum Element<T> {
  /// A fetch is in flight; the token identifies the placeholder so the
  /// result lands in the right slot even if the cache shifted meanwhile.
  Unresolved(u64),
  Resolved(T),
}

fn next_token() -> u64 {
  static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
  NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

type Cache<M> = Vec<Element<Option<M>>>;
type Rows<M> = Rc<Vec<Row<M>>>;

/// One rendered line of the list. Derived from the cache on every rebuild, so
/// the chrome can never go stale (a separator whose neighbour was deleted
/// simply is not produced again).
enum Row<M: Message> {
  /// The top fetch came back empty: history starts here.
  Start,
  /// A fetch is in flight for this slot.
  Loading,
  /// The group below is the first on a new local calendar day.
  DateSeparator(NaiveDate),
  /// Everything below arrived while the user was scrolled up.
  NewDivider,
  Group(MessageGroup<M>),
  /// The bottom fetch came back empty: nothing newer exists (yet).
  End,
}

struct RowBuild<M: Message> {
  rows: Vec<Row<M>>,
  /// How many rows each cache item contributed, index-aligned with the cache.
  rows_per_item: Vec<usize>,
  /// Messages at or below the "new messages" divider.
  unseen: usize,
}

/// Regroup the cache into rows. `day_of` buckets a message into a calendar
/// day; consecutive groups on different days get a separator between them.
/// Separators and the divider always start a new group. Gaps (loading slots,
/// the history ends) reset the day so no separator is drawn across them.
fn build_rows<M: Message>(
  cache: &Cache<M>,
  new_divider: Option<&<M as AsyncListItem>::Identifier>,
  day_of: impl Fn(&M) -> Option<NaiveDate>,
) -> RowBuild<M> {
  let mut rows: Vec<Row<M>> = Vec::new();
  let mut rows_per_item = Vec::with_capacity(cache.len());
  let mut prev_day: Option<NaiveDate> = None;
  let mut unseen = 0;
  let mut past_divider = false;

  for (index, item) in cache.iter().enumerate() {
    let before = rows.len();

    match item {
      Element::Unresolved(_) => {
        rows.push(Row::Loading);
        prev_day = None;
      }
      Element::Resolved(None) => {
        rows.push(if index == 0 { Row::Start } else { Row::End });
        prev_day = None;
      }
      Element::Resolved(Some(message)) => {
        let day = day_of(message);

        if let (Some(prev), Some(day)) = (prev_day, day)
          && prev != day
        {
          rows.push(Row::DateSeparator(day));
        }

        if new_divider.is_some_and(|id| *id == message.get_list_identifier()) {
          rows.push(Row::NewDivider);
          past_divider = true;
        }

        if past_divider {
          unseen += 1;
        }

        match rows.last_mut() {
          Some(Row::Group(group))
            if group.last().get_author().get_identifier() == message.get_author().get_identifier() && message.should_group(group.last()) =>
          {
            group.add(message.clone())
          }
          _ => rows.push(Row::Group(MessageGroup::new(message.clone()))),
        }

        if day.is_some() {
          prev_day = day;
        }
      }
    }

    rows_per_item.push(rows.len() - before);
  }

  RowBuild { rows, rows_per_item, unseen }
}

/// Rows contributed by the cache items `dirty` says were inserted at the top
/// and appended at the bottom, so the carried-over scroll offset can skip
/// them. A trailing end marker is not part of the appended items (appending
/// re-pushes it).
fn carried_rows(rows_per_item: &[usize], has_end_marker: bool, dirty: ListStateDirtyState) -> (usize, usize) {
  let len = rows_per_item.len();
  let top = rows_per_item[..dirty.shift.min(len)].iter().sum();

  let end = len.saturating_sub(has_end_marker as usize);
  let start = end.saturating_sub(dirty.new_items);
  let bottom = rows_per_item[start..end].iter().sum();

  (top, bottom)
}

fn local_day(timestamp: DateTime<Utc>) -> NaiveDate {
  timestamp.with_timezone(&Local).date_naive()
}

/// "Today", "Yesterday", otherwise "August 24, 2026".
fn separator_label(day: NaiveDate, today: NaiveDate) -> String {
  if day == today {
    "Today".to_string()
  } else if day.succ_opt() == Some(today) {
    "Yesterday".to_string()
  } else {
    day.format("%B %-d, %Y").to_string()
  }
}

fn jump_label(unseen: usize) -> SharedString {
  match unseen {
    0 => "Jump to present".into(),
    1 => "1 new message · Jump to present".into(),
    n => format!("{n} new messages · Jump to present").into(),
  }
}

pub struct MessageListComponent<C: Channel + 'static> {
  list: Arc<RwLock<C>>,
  cache: Entity<Cache<C::Message>>,
  overdraw: Pixels,

  /// Set from inside the list render callback when the top / bottom sentinel
  /// rows become visible, i.e. when more history should be requested.
  bounds_flags: Entity<BoundFlags>,

  rows: Rows<C::Message>,
  actions: MessageActions<C::Message>,
  list_state: Option<ListState>,
  /// True while the newest message is on screen; rows that grow later (images
  /// finishing to load) then keep the list pinned to the bottom, like Discord.
  pinned_to_bottom: Rc<Cell<bool>>,
  /// `pinned_to_bottom` as of the last render, to notice the unpinned → pinned
  /// transition (the moment the user has caught up).
  was_pinned: bool,
  list_state_dirty: Option<ListStateDirtyState>,

  /// First message that arrived while the user was scrolled up; the "new
  /// messages" divider is drawn above it until they scroll back down.
  new_divider: Option<<C::Message as AsyncListItem>::Identifier>,
  /// Messages below the divider, shown on the jump-to-present pill.
  unseen: usize,
}

impl<C: Channel + 'static> MessageListComponent<C> {
  pub fn create(cx: &mut Context<Self>, list: C, overdraw: Pixels, actions: MessageActions<C::Message>) -> Self {
    let cache = cx.new(|_| Vec::new());

    cx.observe(&cache, |this, _, cx| {
      this.rebuild(cx);
      cx.notify();
    })
    .detach();

    MessageListComponent {
      list: Arc::new(RwLock::new(list)),
      cache,
      overdraw,
      bounds_flags: cx.new(|_| BoundFlags::default()),
      rows: Rc::new(Vec::new()),
      actions,
      pinned_to_bottom: Rc::new(Cell::new(true)),
      was_pinned: true,
      list_state: None,
      list_state_dirty: None,
      new_divider: None,
      unseen: 0,
    }
  }

  pub fn append_message(&mut self, cx: &mut Context<Self>, message: C::Message) {
    let replaces_pending =
      self.cache.read(cx).iter().any(|item| matches!(item, Element::Resolved(Some(existing)) if existing.get_nonce() == message.get_nonce()));

    if replaces_pending {
      self.cache.update(cx, |cache, cx| {
        if let Some(item) =
          cache.iter_mut().find(|item| matches!(item, Element::Resolved(Some(existing)) if existing.get_nonce() == message.get_nonce()))
        {
          *item = Element::Resolved(Some(message));
          cx.notify();
        }
      });
      return;
    }

    if message.is_own() {
      // Sending follows your own message down, like Discord.
      self.pinned_to_bottom.set(true);
    } else if !self.pinned_to_bottom.get() && self.new_divider.is_none() {
      self.new_divider = Some(message.get_list_identifier());
    }

    self.mark_dirty(ListStateDirtyState { new_items: 1, shift: 0 });

    self.cache.update(cx, |cache, cx| {
      if let Some(Element::Resolved(None)) = cache.last() {
        cache.pop();
      }

      cache.push(Element::Resolved(Some(message)));
      cache.push(Element::Resolved(None));
      cx.notify();
    });
  }

  /// Replace a message in place (edit, reaction change, …), matched by list id.
  pub fn update_message(&mut self, cx: &mut Context<Self>, message: C::Message) {
    let id = message.get_list_identifier();

    self.cache.update(cx, |cache, cx| {
      if let Some(slot) = cache.iter_mut().find(|e| matches!(e, Element::Resolved(Some(m)) if m.get_list_identifier() == id)) {
        *slot = Element::Resolved(Some(message));
        cx.notify();
      }
    });
  }

  pub fn remove_message(&mut self, cx: &mut Context<Self>, id: <C::Message as AsyncListItem>::Identifier) {
    let divider_removed = self.new_divider.as_ref() == Some(&id);
    let mut next_after_divider = None;

    self.cache.update(cx, |cache, cx| {
      let Some(position) = cache.iter().position(|e| matches!(e, Element::Resolved(Some(m)) if m.get_list_identifier() == id)) else {
        return;
      };

      cache.remove(position);

      if divider_removed && let Some(Element::Resolved(Some(next))) = cache.get(position) {
        next_after_divider = Some(next.get_list_identifier());
      }

      cx.notify();
    });

    if divider_removed {
      // The divider moves down to the next unseen message, if any is left.
      self.new_divider = next_after_divider;
    }
  }

  fn mark_dirty(&mut self, change: ListStateDirtyState) {
    let dirty = self.list_state_dirty.get_or_insert_default();
    dirty.shift += change.shift;
    dirty.new_items += change.new_items;
  }

  /// The user is back at the newest message: drop the divider and the count.
  fn caught_up(&mut self, cx: &mut Context<Self>) {
    if self.new_divider.take().is_some() {
      self.rebuild(cx);
    }
  }

  fn jump_to_present(&mut self, cx: &mut Context<Self>) {
    if let Some(state) = &self.list_state {
      state.scroll_to(ListOffset {
        item_ix: state.item_count(),
        offset_in_item: px(0.),
      });
    }

    self.pinned_to_bottom.set(true);
    cx.notify();
  }

  /// Regroup the cache into rows and build a fresh `ListState`, carrying the
  /// scroll position over from the previous one.
  fn rebuild(&mut self, cx: &mut Context<Self>) {
    let dirty = self.list_state_dirty.take().unwrap_or_default();
    let cache = self.cache.read(cx);

    let build = build_rows(cache, self.new_divider.as_ref(), |m| m.get_timestamp().map(local_day));
    let has_end_marker = matches!(cache.last(), Some(Element::Resolved(None)));
    let (shift, added_rows_bottom) = carried_rows(&build.rows_per_item, has_end_marker, dirty);

    let len = build.rows.len();
    let total_items = if len == 0 { 1 } else { len + 2 };
    let new_list_state = ListState::new(total_items, ListAlignment::Bottom, self.overdraw);

    let pinned = self.pinned_to_bottom.clone();
    new_list_state.set_scroll_handler(move |event, _, _| pinned.set(event.visible_range.end >= total_items));

    if let Some(old) = &self.list_state {
      let mut new_scroll_top = old.logical_scroll_top();

      if new_scroll_top.item_ix == old.item_count() {
        new_scroll_top.item_ix += added_rows_bottom;

        if added_rows_bottom > 0 {
          new_scroll_top.offset_in_item = px(0.);
        }
      }

      new_scroll_top.item_ix += shift;
      new_list_state.scroll_to(new_scroll_top);
    }

    self.rows = Rc::new(build.rows);
    self.unseen = build.unseen;
    self.list_state = Some(new_list_state);
  }

  /// Kick off a backend fetch for `index` and write the result into the placeholder tagged `token`.
  fn fetch(cx: &mut Context<Cache<C::Message>>, list: Arc<RwLock<C>>, index: AsyncListIndex<<C::Message as AsyncListItem>::Identifier>, token: u64) {
    cx.spawn(async move |cache, cx| {
      let (tx, rx) = catty::oneshot();

      tokio::spawn(async move {
        let result = list.read().await.get(index).await;
        if tx.send(result).is_err() {
          log::error!("message list went away before fetch completed");
        }
      });

      let Ok(result) = rx.await else { return };

      cache
        .update(cx, |cache, cx| {
          if let Some(item) = cache.iter_mut().find(|e| matches!(e, Element::Unresolved(t) if *t == token)) {
            *item = Element::Resolved(result.map(|v| v.content));
          }
          cx.notify();
        })
        .ok();
    })
    .detach();
  }

  /// Request more history in whichever direction the sentinel rows asked for.
  fn update(&mut self, cx: &mut Context<Self>) {
    let mut dirty = None;
    let mut flags = *self.bounds_flags.read(cx);

    if flags.after {
      let list = self.list.clone();

      self.cache.update(cx, |cache, cx| {
        let index = match cache.last() {
          None => AsyncListIndex::RelativeToBottom(0),
          Some(Element::Resolved(Some(v))) => AsyncListIndex::After(v.get_list_identifier()),
          Some(_) => {
            flags.after = false;
            return;
          }
        };

        let token = next_token();
        cache.push(Element::Unresolved(token));
        Self::fetch(cx, list, index, token);

        dirty = Some(ListStateDirtyState { new_items: 1, shift: 0 });
      });
    }

    if flags.before {
      let list = self.list.clone();

      self.cache.update(cx, |cache, cx| {
        let index = match cache.first() {
          Some(Element::Resolved(Some(v))) => AsyncListIndex::Before(v.get_list_identifier()),
          _ => {
            flags.before = false;
            return;
          }
        };

        let token = next_token();
        cache.insert(0, Element::Unresolved(token));
        Self::fetch(cx, list, index, token);

        let mut v = dirty.unwrap_or_default();
        v.shift += 1;
        dirty = Some(v);
      });
    }

    if let Some(dirty) = dirty {
      self.mark_dirty(dirty);
      cx.notify();
    }

    self.bounds_flags.update(cx, |v, _| {
      if flags.after {
        v.after = false;
      }

      if flags.before {
        v.before = false;
      }
    });
  }

  /// Floating "Jump to present" pill, bottom-right, shown while scrolled up.
  fn jump_pill(&self, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .id("jump-to-present")
      .absolute()
      .bottom(px(PILL_INSET))
      .right(px(PILL_INSET))
      .h(px(24.))
      .px(px(10.))
      .flex()
      .items_center()
      .rounded(tokens::RADIUS_150)
      .bg(tokens::BRAND)
      .hover(|style| style.bg(tokens::BRAND_HOVER))
      .cursor_pointer()
      .text_size(tokens::TYPE_S)
      .line_height(px(16.))
      .font_weight(FontWeight::MEDIUM)
      .text_color(white())
      .whitespace_nowrap()
      .child(jump_label(self.unseen))
      .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.jump_to_present(cx)))
  }
}

impl<C: Channel + 'static> Render for MessageListComponent<C> {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    self.update(cx);

    let pinned = self.pinned_to_bottom.get();
    if pinned && !self.was_pinned {
      self.caught_up(cx);
    }
    self.was_pinned = pinned;

    if self.list_state.is_none() {
      self.rebuild(cx);
    }

    let state = self.list_state.clone().expect("list state is built above");

    if pinned {
      state.scroll_to(ListOffset {
        item_ix: state.item_count(),
        offset_in_item: px(0.),
      });
    }

    let rows = self.rows.clone();
    let bounds = self.bounds_flags.clone();
    let actions = self.actions.clone();
    let len = rows.len();
    let today = Local::now().date_naive();

    let list = list(state, move |idx, window, cx| {
      if len == 0 {
        bounds.update(cx, |v, _| v.after = true);
        window.request_animation_frame();
        return div().into_any_element();
      }

      if idx == 0 {
        bounds.update(cx, |v, _| v.before = true);
        window.request_animation_frame();
        div().into_any_element()
      } else if idx == len + 1 {
        bounds.update(cx, |v, _| v.after = true);
        window.request_animation_frame();
        div().into_any_element()
      } else {
        match &rows[idx - 1] {
          Row::Start => start_row().into_any_element(),
          Row::Loading => loading_row().into_any_element(),
          Row::DateSeparator(day) => date_separator(separator_label(*day, today)).into_any_element(),
          Row::NewDivider => new_divider().into_any_element(),
          Row::Group(group) => message_group(group.clone(), actions.clone(), window, cx).into_any_element(),
          Row::End => div().into_any_element(),
        }
      }
    });

    div().size_full().relative().bg(tokens::BG_SECONDARY).child(list.size_full()).children((!pinned).then(|| self.jump_pill(cx)))
  }
}

/// "This is the beginning of the conversation." above the oldest message.
fn start_row() -> impl IntoElement {
  div()
    .w_full()
    .pt(px(START_GAP))
    .pb(px(SEPARATOR_PAD_Y))
    .flex()
    .justify_center()
    .text_size(tokens::TYPE_S)
    .line_height(px(16.))
    .font_weight(FontWeight::MEDIUM)
    .text_color(tokens::TEXT_TERTIARY)
    .child("This is the beginning of the conversation.")
}

/// 1px rules either side of a centred day label.
fn date_separator(label: String) -> impl IntoElement {
  let rule = || div().flex_1().h(px(1.)).bg(tokens::BORDER);

  div()
    .w_full()
    .h(px(SEPARATOR_HEIGHT))
    .py(px(SEPARATOR_PAD_Y))
    .pl(px(ROW_PAD_LEFT))
    .pr(px(ROW_PAD_RIGHT))
    .flex()
    .flex_row()
    .items_center()
    .gap(px(8.))
    .child(rule())
    .child(
      div()
        .flex_shrink_0()
        .text_size(tokens::TYPE_S)
        .line_height(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(tokens::TEXT_TERTIARY)
        .whitespace_nowrap()
        .child(label),
    )
    .child(rule())
}

/// Thin brand line with a "NEW" pill at its right end.
fn new_divider() -> impl IntoElement {
  div()
    .w_full()
    .h(px(NEW_DIVIDER_HEIGHT))
    .pl(px(ROW_PAD_LEFT))
    .pr(px(ROW_PAD_RIGHT))
    .flex()
    .flex_row()
    .items_center()
    .child(div().flex_1().h(px(1.)).bg(tokens::BRAND))
    .child(
      div()
        .flex_shrink_0()
        .h(px(12.))
        .px(px(4.))
        .flex()
        .items_center()
        .rounded(tokens::RADIUS_050)
        .bg(tokens::BRAND)
        .text_size(tokens::TYPE_XS)
        .line_height(px(12.))
        .font_weight(FontWeight::BOLD)
        .text_color(white())
        .child("NEW"),
    )
}

#[cfg(test)]
mod tests {
  use chrono::TimeZone;
  use gpui::App;
  use scope_chat::message::IconRenderConfig;
  use scope_rich::RichContentView;

  use super::*;

  #[derive(Clone, PartialEq, Eq)]
  struct Author(u64);

  impl MessageAuthor for Author {
    type Identifier = u64;
    type DisplayName = SharedString;
    type Icon = SharedString;

    fn get_display_name(&self) -> SharedString {
      format!("user {}", self.0).into()
    }

    fn get_icon(&self, _config: IconRenderConfig) -> SharedString {
      SharedString::default()
    }

    fn get_identifier(&self) -> u64 {
      self.0
    }
  }

  #[derive(Clone)]
  struct Msg {
    id: u64,
    author: u64,
    at: DateTime<Utc>,
  }

  impl AsyncListItem for Msg {
    type Identifier = u64;

    fn get_list_identifier(&self) -> u64 {
      self.id
    }
  }

  impl Message for Msg {
    type Identifier = u64;
    type Author = Author;

    fn get_author(&self) -> Author {
      Author(self.author)
    }

    fn is_own(&self) -> bool {
      false
    }

    fn get_content(&self, _window: &mut Window, _cx: &mut App) -> Entity<RichContentView> {
      unreachable!("rows are not rendered in unit tests")
    }

    fn get_identifier(&self) -> Option<u64> {
      Some(self.id)
    }

    fn get_nonce(&self) -> impl PartialEq {
      self.id
    }

    fn should_group(&self, previous: &Self) -> bool {
      (self.at - previous.at).num_minutes().abs() <= 5
    }

    fn get_timestamp(&self) -> Option<DateTime<Utc>> {
      Some(self.at)
    }
  }

  /// A message by `author` at `day` of August 2026, `minute` minutes past noon UTC.
  fn msg(id: u64, author: u64, day: u32, minute: u32) -> Element<Option<Msg>> {
    let at = Utc.with_ymd_and_hms(2026, 8, day, 12, minute, 0).unwrap();
    Element::Resolved(Some(Msg { id, author, at }))
  }

  fn utc_day(message: &Msg) -> Option<NaiveDate> {
    Some(message.at.date_naive())
  }

  fn describe(rows: &[Row<Msg>]) -> Vec<String> {
    rows
      .iter()
      .map(|row| match row {
        Row::Start => "start".to_string(),
        Row::Loading => "loading".to_string(),
        Row::DateSeparator(day) => format!("sep {day}"),
        Row::NewDivider => "new".to_string(),
        Row::Group(group) => format!(
          "group[{}]",
          group.messages().iter().map(|m| m.id.to_string()).collect::<Vec<_>>().join(",")
        ),
        Row::End => "end".to_string(),
      })
      .collect()
  }

  #[test]
  fn separator_between_groups_on_different_days() {
    let cache = vec![
      Element::Resolved(None),
      msg(1, 7, 23, 0),
      msg(2, 7, 23, 1),
      msg(3, 7, 24, 0),
      msg(4, 8, 24, 2),
      Element::Resolved(None),
    ];
    let build = build_rows(&cache, None, utc_day);

    assert_eq!(
      describe(&build.rows),
      ["start", "group[1,2]", "sep 2026-08-24", "group[3]", "group[4]", "end"]
    );
    assert_eq!(build.rows_per_item, [1, 1, 0, 2, 1, 1]);
    assert_eq!(build.unseen, 0);
  }

  #[test]
  fn same_day_different_authors_get_no_separator() {
    let cache = vec![msg(1, 7, 24, 0), msg(2, 8, 24, 1)];
    let build = build_rows(&cache, None, utc_day);

    assert_eq!(describe(&build.rows), ["group[1]", "group[2]"]);
  }

  #[test]
  fn no_separator_across_a_loading_slot() {
    let cache = vec![msg(1, 7, 23, 0), Element::Unresolved(9), msg(2, 7, 24, 0)];
    let build = build_rows(&cache, None, utc_day);

    assert_eq!(describe(&build.rows), ["group[1]", "loading", "group[2]"]);
  }

  #[test]
  fn lone_end_marker_is_the_start_of_history() {
    let build = build_rows(&vec![Element::Resolved(None)], None, utc_day);
    assert_eq!(describe(&build.rows), ["start"]);

    let build = build_rows(&vec![msg(1, 7, 24, 0), Element::Resolved(None)], None, utc_day);
    assert_eq!(describe(&build.rows), ["group[1]", "end"]);
  }

  #[test]
  fn new_divider_breaks_the_group_and_counts_unseen() {
    let cache = vec![msg(1, 7, 24, 0), msg(2, 7, 24, 1), msg(3, 7, 24, 2), Element::Resolved(None)];
    let build = build_rows(&cache, Some(&2), utc_day);

    assert_eq!(describe(&build.rows), ["group[1]", "new", "group[2,3]", "end"]);
    assert_eq!(build.unseen, 2);
  }

  #[test]
  fn separator_comes_before_the_new_divider() {
    let cache = vec![msg(1, 7, 23, 0), msg(2, 7, 24, 0)];
    let build = build_rows(&cache, Some(&2), utc_day);

    assert_eq!(describe(&build.rows), ["group[1]", "sep 2026-08-24", "new", "group[2]"]);
    assert_eq!(build.rows_per_item, [1, 3]);
  }

  #[test]
  fn missing_divider_message_draws_nothing() {
    let cache = vec![msg(1, 7, 24, 0), msg(2, 7, 24, 1)];
    let build = build_rows(&cache, Some(&99), utc_day);

    assert_eq!(describe(&build.rows), ["group[1,2]"]);
    assert_eq!(build.unseen, 0);
  }

  #[test]
  fn removing_the_message_at_a_day_boundary_drops_the_separator() {
    let mut cache = vec![msg(1, 7, 23, 0), msg(2, 7, 24, 0), Element::Resolved(None)];
    assert_eq!(
      describe(&build_rows(&cache, None, utc_day).rows),
      ["group[1]", "sep 2026-08-24", "group[2]", "end"]
    );

    cache.retain(|e| !matches!(e, Element::Resolved(Some(m)) if m.id == 2));
    assert_eq!(describe(&build_rows(&cache, None, utc_day).rows), ["group[1]", "end"]);
  }

  #[test]
  fn carried_rows_count_rows_not_cache_items() {
    // [loading, msg(+sep), msg, end]: one slot inserted at the top.
    let rows_per_item = [1, 2, 1, 1];
    assert_eq!(carried_rows(&rows_per_item, true, ListStateDirtyState { shift: 1, new_items: 0 }), (1, 0));

    // [msg, msg, msg(+sep +new), end]: one message appended under a re-pushed end marker.
    let rows_per_item = [1, 1, 3, 1];
    assert_eq!(carried_rows(&rows_per_item, true, ListStateDirtyState { shift: 0, new_items: 1 }), (0, 3));

    // [msg, msg, loading]: a bottom fetch in flight, no end marker.
    let rows_per_item = [1, 1, 1];
    assert_eq!(
      carried_rows(&rows_per_item, false, ListStateDirtyState { shift: 0, new_items: 1 }),
      (0, 1)
    );

    assert_eq!(carried_rows(&[], true, ListStateDirtyState { shift: 1, new_items: 1 }), (0, 0));
  }

  #[test]
  fn separator_labels() {
    let today = NaiveDate::from_ymd_opt(2026, 8, 24).unwrap();

    assert_eq!(separator_label(today, today), "Today");
    assert_eq!(separator_label(today.pred_opt().unwrap(), today), "Yesterday");
    assert_eq!(separator_label(NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(), today), "August 1, 2026");
    assert_eq!(
      separator_label(NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(), today),
      "December 31, 2025"
    );
  }

  #[test]
  fn jump_labels() {
    assert_eq!(jump_label(0), "Jump to present");
    assert_eq!(jump_label(1), "1 new message · Jump to present");
    assert_eq!(jump_label(4), "4 new messages · Jump to present");
  }
}
