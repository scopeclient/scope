//! Bottom-anchored, batch-paged message list.
//!
//! Messages live in `items`, oldest first. Each edge (top = older history,
//! bottom = newer) has an explicit [`EdgeState`]; at most one load runs per
//! edge, and a whole page lands as a single update. Rendering derives `rows`
//! (groups plus chrome: date separators, the "new messages" divider, the
//! start-of-history note) from `items` on every change, with the scroll
//! offset carried over in rendered rows.

use std::{cell::Cell, collections::HashSet, rc::Rc, sync::Arc};

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

/// Horizontal insets of the separator rules; match the message row padding.
const ROW_PAD_LEFT: f32 = 17.;
const ROW_PAD_RIGHT: f32 = 16.;
const SEPARATOR_HEIGHT: f32 = 28.;
const SEPARATOR_PAD_Y: f32 = 8.;
const START_GAP: f32 = 24.;
const PILL_INSET: f32 = 12.;

/// Messages fetched when a channel opens.
const INITIAL_LOAD: usize = 50;
/// Messages fetched per scroll-up trigger.
const OLDER_LOAD: usize = 50;

#[derive(Clone, Copy, PartialEq, Eq)]
enum EdgeState {
  /// Nothing in flight; a visible edge sentinel may start a load.
  Idle,
  Loading,
  /// History ends here; never fetch this edge again.
  End,
}

/// What changed since the last rebuild, in items: `shift` inserted at the
/// top, `new_items` appended at the bottom.
#[derive(Clone, Copy, Default)]
struct DirtyState {
  new_items: usize,
  shift: usize,
}

#[derive(Clone, Copy, Default)]
struct BoundFlags {
  before: bool,
  after: bool,
}

type Rows<M> = Rc<Vec<Row<M>>>;

enum Row<M: Message> {
  /// History starts here ("this is the beginning of the conversation").
  Start,
  /// An edge load is in flight.
  Loading,
  /// The group below is the first on a new local calendar day.
  DateSeparator(NaiveDate),
  /// Everything below arrived while the user was scrolled up.
  NewDivider,
  Group(MessageGroup<M>),
}

struct RowBuild<M: Message> {
  rows: Vec<Row<M>>,
  /// Rows contributed per item, index-aligned with `items`; edge chrome
  /// (Start/Loading) is accounted to no item.
  rows_per_item: Vec<usize>,
  /// Messages at or below the "new messages" divider.
  unseen: usize,
  /// Rows before the first item's rows (top chrome).
  top_chrome: usize,
}

fn build_rows<M: Message>(
  items: &[M],
  top: EdgeState,
  top_reached: bool,
  bottom: EdgeState,
  new_divider: Option<&<M as AsyncListItem>::Identifier>,
  day_of: impl Fn(&M) -> Option<NaiveDate>,
) -> RowBuild<M> {
  let mut rows: Vec<Row<M>> = Vec::new();

  if top_reached {
    rows.push(Row::Start);
  } else if top == EdgeState::Loading {
    rows.push(Row::Loading);
  }

  let top_chrome = rows.len();
  let mut rows_per_item = Vec::with_capacity(items.len());
  let mut prev_day: Option<NaiveDate> = None;
  let mut unseen = 0;
  let mut past_divider = false;

  for message in items {
    let before = rows.len();
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

    rows_per_item.push(rows.len() - before);
  }

  if bottom == EdgeState::Loading {
    rows.push(Row::Loading);
  }

  RowBuild {
    rows,
    rows_per_item,
    unseen,
    top_chrome,
  }
}

/// Rendered rows contributed by the `dirty.shift` items inserted at the top
/// and the `dirty.new_items` appended at the bottom.
fn carried_rows(rows_per_item: &[usize], dirty: DirtyState) -> (usize, usize) {
  let len = rows_per_item.len();
  let top: usize = rows_per_item[..dirty.shift.min(len)].iter().sum();
  let start = len.saturating_sub(dirty.new_items);
  let bottom: usize = rows_per_item[start..].iter().sum();
  (top, bottom)
}

fn local_day(timestamp: DateTime<Utc>) -> NaiveDate {
  timestamp.with_timezone(&Local).date_naive()
}

/// "today", "yesterday", otherwise "august 24, 2026".
fn separator_label(day: NaiveDate, today: NaiveDate) -> String {
  if day == today {
    "today".to_string()
  } else if day.succ_opt() == Some(today) {
    "yesterday".to_string()
  } else {
    day.format("%B %-d, %Y").to_string().to_lowercase()
  }
}

fn jump_label(unseen: usize) -> SharedString {
  match unseen {
    0 => "jump to present".into(),
    1 => "1 new message · jump to present".into(),
    n => format!("{n} new messages · jump to present").into(),
  }
}

/// One finished edge load: messages to splice in and whether history ended.
struct LoadedPage<M> {
  /// Oldest → newest, ready to splice.
  messages: Vec<M>,
  reached_end: bool,
}

pub struct MessageListComponent<C: Channel + 'static> {
  list: Arc<RwLock<C>>,
  overdraw: Pixels,
  actions: MessageActions<C::Message>,

  /// Oldest first.
  items: Vec<C::Message>,
  top_state: EdgeState,
  /// The oldest loaded message is the first that ever existed.
  top_reached: bool,
  bottom_state: EdgeState,
  /// Whether the newest loaded message is the newest that exists. Live
  /// messages keep this true; it only goes false after jumping into history.
  bottom_reached: bool,
  /// Set once the initial load has been kicked off.
  started: bool,

  /// Edge sentinels flip these from inside the list's render callback.
  bounds_flags: Entity<BoundFlags>,

  rows: Rows<C::Message>,
  list_state: Option<ListState>,
  /// False as soon as the user scrolls away from the bottom (driven by real
  /// scroll events); while true, growth keeps the view glued to the newest
  /// message, like Discord.
  pinned_to_bottom: Rc<Cell<bool>>,
  was_pinned: bool,
  /// One-shot: scroll hard to the bottom on the next render.
  scroll_to_bottom: Cell<bool>,
  dirty: Option<DirtyState>,

  /// First message that arrived while the user was scrolled up.
  new_divider: Option<<C::Message as AsyncListItem>::Identifier>,
  unseen: usize,
}

impl<C: Channel + 'static> MessageListComponent<C> {
  pub fn create(cx: &mut Context<Self>, list: C, overdraw: Pixels, actions: MessageActions<C::Message>) -> Self {
    MessageListComponent {
      list: Arc::new(RwLock::new(list)),
      overdraw,
      actions,
      items: Vec::new(),
      top_state: EdgeState::Idle,
      top_reached: false,
      bottom_state: EdgeState::Idle,
      bottom_reached: false,
      started: false,
      bounds_flags: cx.new(|_| BoundFlags::default()),
      rows: Rc::new(Vec::new()),
      list_state: None,
      pinned_to_bottom: Rc::new(Cell::new(true)),
      was_pinned: true,
      scroll_to_bottom: Cell::new(false),
      dirty: None,
      new_divider: None,
      unseen: 0,
    }
  }

  fn known_ids(&self) -> HashSet<<C::Message as AsyncListItem>::Identifier> {
    self.items.iter().map(|m| m.get_list_identifier()).collect()
  }

  pub fn append_message(&mut self, cx: &mut Context<Self>, message: C::Message) {
    // A confirmed echo replaces its optimistic bubble, matched by nonce.
    if let Some(slot) = self.items.iter_mut().find(|existing| existing.get_nonce() == message.get_nonce()) {
      *slot = message;
      self.rebuild(cx);
      cx.notify();
      return;
    }

    // Live duplicate of something already paged in.
    if self.items.iter().any(|m| m.get_list_identifier() == message.get_list_identifier()) {
      return;
    }

    if message.is_own() {
      // Sending follows your own message down, like Discord.
      self.pinned_to_bottom.set(true);
      self.scroll_to_bottom.set(true);
    } else if !self.pinned_to_bottom.get() && self.new_divider.is_none() {
      self.new_divider = Some(message.get_list_identifier());
    }

    self.mark_dirty(DirtyState { new_items: 1, shift: 0 });
    self.items.push(message);
    self.rebuild(cx);
    cx.notify();
  }

  /// Replace a message in place (edit, reaction change, …), matched by list id.
  pub fn update_message(&mut self, cx: &mut Context<Self>, message: C::Message) {
    let id = message.get_list_identifier();

    if let Some(slot) = self.items.iter_mut().find(|m| m.get_list_identifier() == id) {
      *slot = message;
      self.rebuild(cx);
      cx.notify();
    }
  }

  pub fn remove_message(&mut self, cx: &mut Context<Self>, id: <C::Message as AsyncListItem>::Identifier) {
    let Some(position) = self.items.iter().position(|m| m.get_list_identifier() == id) else {
      return;
    };

    self.items.remove(position);

    if self.new_divider.as_ref() == Some(&id) {
      // The divider moves down to the next unseen message, if any is left.
      self.new_divider = self.items.get(position).map(|next| next.get_list_identifier());
    }

    self.rebuild(cx);
    cx.notify();
  }

  fn mark_dirty(&mut self, change: DirtyState) {
    let dirty = self.dirty.get_or_insert_default();
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
    self.pinned_to_bottom.set(true);
    self.scroll_to_bottom.set(true);
    cx.notify();
  }

  /// Newest → older chain: fetch the newest message, then walk `Before` until
  /// `count` messages or the top of history.
  fn load_initial(&mut self, cx: &mut Context<Self>) {
    self.started = true;
    self.top_state = EdgeState::Loading;

    let list = self.list.clone();

    cx.spawn(async move |this, cx| {
      let (tx, rx) = catty::oneshot();

      tokio::spawn(async move {
        let guard = list.read().await;
        let mut collected = Vec::new();
        let mut reached_end = false;

        match guard.get(AsyncListIndex::RelativeToBottom(0)).await {
          None => reached_end = true,
          Some(newest) => {
            let top = newest.is_top;
            collected.push(newest.content);
            reached_end = top;

            while !reached_end && collected.len() < INITIAL_LOAD {
              let anchor = collected.last().expect("just pushed").get_list_identifier();
              match guard.get(AsyncListIndex::Before(anchor)).await {
                None => reached_end = true,
                Some(older) => {
                  reached_end = older.is_top;
                  collected.push(older.content);
                }
              }
            }
          }
        }

        // Collected newest → oldest; the list wants oldest first.
        collected.reverse();
        let _ = tx.send(LoadedPage {
          messages: collected,
          reached_end,
        });
      });

      let Ok(page) = rx.await else { return };

      this
        .update(cx, |this, cx| {
          let known = this.known_ids();
          let fresh: Vec<_> = page.messages.into_iter().filter(|m| !known.contains(&m.get_list_identifier())).collect();

          this.mark_dirty(DirtyState {
            new_items: fresh.len(),
            shift: 0,
          });
          // Live messages may already sit in `items`; history goes before them.
          let mut items = fresh;
          items.append(&mut this.items);
          this.items = items;

          this.top_reached = page.reached_end;
          this.top_state = if page.reached_end { EdgeState::End } else { EdgeState::Idle };
          this.bottom_reached = true;
          this.scroll_to_bottom.set(true);
          this.rebuild(cx);
          cx.notify();
        })
        .unwrap_or_else(|_| log::debug!("channel closed before its history arrived"));
    })
    .detach();
  }

  /// Fetch up to [`OLDER_LOAD`] messages above the oldest confirmed one.
  fn load_older(&mut self, cx: &mut Context<Self>) {
    if self.top_state != EdgeState::Idle {
      return;
    }

    // Optimistic messages have no server identity; never page relative to one.
    let Some(anchor) = self.items.iter().find(|m| m.get_identifier().is_some()).map(|m| m.get_list_identifier()) else {
      return;
    };

    self.top_state = EdgeState::Loading;
    self.rebuild(cx);
    cx.notify();

    let list = self.list.clone();

    cx.spawn(async move |this, cx| {
      let (tx, rx) = catty::oneshot();

      tokio::spawn(async move {
        let guard = list.read().await;
        let mut collected = Vec::new();
        let mut reached_end = false;
        let mut anchor = anchor;

        while !reached_end && collected.len() < OLDER_LOAD {
          match guard.get(AsyncListIndex::Before(anchor)).await {
            None => reached_end = true,
            Some(older) => {
              reached_end = older.is_top;
              anchor = older.content.get_list_identifier();
              collected.push(older.content);
            }
          }
        }

        collected.reverse();
        let _ = tx.send(LoadedPage {
          messages: collected,
          reached_end,
        });
      });

      let Ok(page) = rx.await else { return };

      this
        .update(cx, |this, cx| {
          let known = this.known_ids();
          let fresh: Vec<_> = page.messages.into_iter().filter(|m| !known.contains(&m.get_list_identifier())).collect();

          this.mark_dirty(DirtyState {
            new_items: 0,
            shift: fresh.len(),
          });
          let mut items = fresh;
          items.append(&mut this.items);
          this.items = items;

          this.top_reached = page.reached_end;
          this.top_state = if page.reached_end { EdgeState::End } else { EdgeState::Idle };
          this.rebuild(cx);
          cx.notify();
        })
        .unwrap_or_else(|_| log::debug!("channel closed before older history arrived"));
    })
    .detach();
  }

  /// Start whichever loads the visible edge sentinels asked for.
  fn service_edges(&mut self, cx: &mut Context<Self>) {
    let flags = *self.bounds_flags.read(cx);

    if flags.before || flags.after {
      self.bounds_flags.update(cx, |v, _| *v = BoundFlags::default());
    }

    if !self.started {
      self.load_initial(cx);
      return;
    }

    if flags.before && !self.top_reached {
      self.load_older(cx);
    }

    // Newer-than-loaded paging (after jumping into deep history) would go
    // here; live messages arrive through the channel events instead.
  }

  /// Regroup and rebuild the `ListState`, carrying the scroll position over.
  fn rebuild(&mut self, cx: &mut Context<Self>) {
    let dirty = self.dirty.take().unwrap_or_default();

    let build = build_rows(
      &self.items,
      self.top_state,
      self.top_reached,
      self.bottom_state,
      self.new_divider.as_ref(),
      |m| m.get_timestamp().map(local_day),
    );
    let (shift, added_rows_bottom) = carried_rows(&build.rows_per_item, dirty);

    let len = build.rows.len();
    let total_items = if len == 0 { 1 } else { len + 2 };
    let new_list_state = ListState::new(total_items, ListAlignment::Bottom, self.overdraw);

    let pinned = self.pinned_to_bottom.clone();
    new_list_state.set_scroll_handler(move |event, _, _| pinned.set(!event.is_scrolled));

    if let Some(old) = &self.list_state {
      let mut new_scroll_top = old.logical_scroll_top();

      if new_scroll_top.item_ix >= old.item_count() {
        new_scroll_top.item_ix += added_rows_bottom;

        if added_rows_bottom > 0 {
          new_scroll_top.offset_in_item = px(0.);
        }
      }

      new_scroll_top.item_ix += shift;
      new_list_state.scroll_to(new_scroll_top);
    }

    let _ = build.top_chrome;
    self.rows = Rc::new(build.rows);
    self.unseen = build.unseen;
    self.list_state = Some(new_list_state);
  }

  /// Floating "jump to present" pill, bottom-right, shown while scrolled up.
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
    self.service_edges(cx);

    let pinned = self.pinned_to_bottom.get();
    if pinned && !self.was_pinned {
      self.caught_up(cx);
    }
    self.was_pinned = pinned;

    if self.list_state.is_none() {
      self.rebuild(cx);
    }

    let state = self.list_state.clone().expect("list state is built above");

    if self.scroll_to_bottom.take() || pinned {
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
        }
      }
    });

    div().size_full().relative().bg(tokens::BG_SECONDARY).child(list.size_full()).children((!pinned).then(|| self.jump_pill(cx)))
  }
}

/// "this is the beginning of the conversation" above the oldest message.
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
    .child("this is the beginning of the conversation")
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
    .items_center()
    .gap(px(8.))
    .child(rule())
    .child(div().text_size(tokens::TYPE_S).line_height(px(16.)).font_weight(FontWeight::MEDIUM).text_color(tokens::TEXT_TERTIARY).child(label))
    .child(rule())
}

/// Brand-coloured rule with a right-aligned "new" tag.
fn new_divider() -> impl IntoElement {
  div()
    .w_full()
    .h(px(16.))
    .pl(px(ROW_PAD_LEFT))
    .pr(px(ROW_PAD_RIGHT))
    .flex()
    .items_center()
    .gap(px(6.))
    .child(div().flex_1().h(px(1.)).bg(tokens::BRAND))
    .child(
      div()
        .px(px(4.))
        .rounded(px(2.))
        .bg(tokens::BRAND)
        .text_size(px(10.))
        .line_height(px(14.))
        .font_weight(FontWeight::BOLD)
        .text_color(white())
        .child("new"),
    )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn separator_labels() {
    let today = NaiveDate::from_ymd_opt(2026, 8, 24).unwrap();
    assert_eq!(separator_label(today, today), "today");
    assert_eq!(separator_label(today.pred_opt().unwrap(), today), "yesterday");
    assert_eq!(separator_label(NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(), today), "august 1, 2026");
    assert_eq!(
      separator_label(NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(), today),
      "december 31, 2025"
    );
  }

  #[test]
  fn jump_labels() {
    assert_eq!(jump_label(0), "jump to present");
    assert_eq!(jump_label(1), "1 new message · jump to present");
    assert_eq!(jump_label(7), "7 new messages · jump to present");
  }

  #[test]
  fn carried_rows_counts_edges() {
    // items contributed 1, 2 (separator + group), 1, 1 rows
    let per_item = [1, 2, 1, 1];
    let (top, bottom) = carried_rows(&per_item, DirtyState { shift: 2, new_items: 1 });
    assert_eq!(top, 3);
    assert_eq!(bottom, 1);

    let (top, bottom) = carried_rows(&per_item, DirtyState::default());
    assert_eq!((top, bottom), (0, 0));

    // shift larger than the list clamps
    let (top, _) = carried_rows(&per_item, DirtyState { shift: 10, new_items: 0 });
    assert_eq!(top, 5);
  }
}
