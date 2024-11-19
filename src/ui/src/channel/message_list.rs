use std::sync::Arc;

use gpui::{div, list, rgb, Context, IntoElement, ListAlignment, ListState, Model, ParentElement, Pixels, Render, Styled, ViewContext};
use scope_chat::{
  async_list::{AsyncListIndex, AsyncListItem},
  channel::Channel,
  message::Message,
};
use tokio::sync::RwLock;

use super::message::{message, MessageGroup};

#[derive(Clone, Copy)]
struct ListStateDirtyState {
  pub new_items: usize,
  pub shift: usize,
}

#[derive(Clone, Copy)]
struct BoundFlags {
  pub before: bool,
  pub after: bool,
}

#[derive(Debug)]
pub enum Element<T> {
  Unresolved,
  Resolved(T),
}

pub struct MessageListComponent<C: Channel>
where
  C::Content: 'static,
{
  list: Arc<RwLock<C>>,
  cache: Model<Vec<Element<Option<C::Content>>>>,
  overdraw: Pixels,

  // top, bottom
  bounds_flags: Model<BoundFlags>,

  list_state: Model<Option<ListState>>,
  list_state_dirty: Model<Option<ListStateDirtyState>>,
}

pub enum StartAt {
  Bottom,
  Top,
}

impl<T: Channel> MessageListComponent<T>
where
  T: 'static,
{
  pub fn create(cx: &mut ViewContext<Self>, list: T, overdraw: Pixels) -> Self {
    let cache = cx.new_model(|_| Default::default());
    let list_state = cx.new_model(|_| None);
    let list_state_dirty = cx.new_model(|_| None);

    let lsc = list_state.clone();

    cx.observe(&cache, move |c, _, cx| {
      let ls = c.list_state(cx);

      lsc.update(cx, |v, _| *v = Some(ls));

      cx.notify();
    })
    .detach();

    let lsc = list_state.clone();

    cx.observe(&list_state_dirty, move |c, _, cx| {
      let ls = c.list_state(cx);

      lsc.update(cx, |v, _| *v = Some(ls));

      cx.notify();
    })
    .detach();

    MessageListComponent {
      list: Arc::new(RwLock::new(list)),
      cache,
      overdraw,
      bounds_flags: cx.new_model(|_| BoundFlags { before: false, after: false }),
      list_state,
      list_state_dirty,
    }
  }

  pub fn append_message(&mut self, cx: &mut ViewContext<Self>, message: T::Message) {
    self.cache.update(cx, |borrow, cx| {
      for item in borrow.iter_mut() {
        if let Element::Resolved(Some(haystack)) = item {
          if haystack.get_nonce() == message.get_nonce() {
            *item = Element::Resolved(Some(message));

            cx.notify();
            return;
          }
        }
      }

      if let Some(Element::Resolved(None)) = borrow.last() {
        borrow.pop();
      }

      borrow.push(Element::Resolved(Some(message)));
      borrow.push(Element::Resolved(None));

      cx.update_model(&self.list_state_dirty, |v, _| *v = Some(ListStateDirtyState { new_items: 1, shift: 0 }));

      cx.notify();
    });
  }

  fn list_state(&self, cx: &mut gpui::ViewContext<Self>) -> ListState {
    let bounds_model = self.bounds_flags.clone();

    let list_state_dirty = *self.list_state_dirty.read(cx);

    let mut added_elements_bottom = 0;
    let mut shift = 0;

    let mut remaining_shift = list_state_dirty.map(|v| v.shift).unwrap_or(0);
    let mut remaining_gap_new_items = self.cache.read(cx).len() - list_state_dirty.map(|v| v.new_items).unwrap_or(0);

    let mut groups = vec![];

    for (item, index) in self.cache.read(cx).iter().zip(0..) {
      let mut items_added: usize = 0;

      match item {
        Element::Unresolved => groups.push(Element::Unresolved),
        Element::Resolved(None) => groups.push(Element::Resolved(None)),
        Element::Resolved(Some(m)) => match groups.last_mut() {
          None | Some(Element::Unresolved) | Some(Element::Resolved(None)) => {
            items_added += 1;
            groups.push(Element::Resolved(Some(MessageGroup::new(m.clone()))));
          }
          Some(Element::Resolved(Some(old_group))) => {
            if m.get_author() == old_group.last().get_author() && m.should_group(old_group.last()) {
              old_group.add(m.clone());
            } else {
              items_added += 1;
              groups.push(Element::Resolved(Some(MessageGroup::new(m.clone()))));
            }
          }
        },
      }

      if index == 0 {
        continue;
      }

      if remaining_shift > 0 {
        remaining_shift -= 1;
        shift += items_added;
      }

      if remaining_gap_new_items == 0 {
        added_elements_bottom = items_added;
      } else {
        remaining_gap_new_items -= 1;
      }
    }

    let len = groups.len();

    let new_list_state = ListState::new(
      if len == 0 { 1 } else { len + 2 },
      ListAlignment::Bottom,
      self.overdraw,
      move |idx, cx| {
        if len == 0 {
          cx.update_model(&bounds_model, |v, _| v.after = true);

          return div().into_any_element();
        }

        if idx == 0 {
          cx.update_model(&bounds_model, |v, _| v.before = true);

          div()
        } else if idx == len + 1 {
          cx.update_model(&bounds_model, |v, _| v.after = true);

          div()
        } else {
          match &groups[idx - 1] {
            Element::Unresolved => div().text_color(rgb(0xFFFFFF)).child("Loading..."),
            Element::Resolved(None) => div(), // we've hit the ends
            Element::Resolved(Some(group)) => div().child(message(group.clone())),
          }
        }
        .into_any_element()
      },
    );

    let old_list_state = self.list_state.read(cx);

    if let Some(old_list_state) = old_list_state {
      let mut new_scroll_top = old_list_state.logical_scroll_top();

      if old_list_state.logical_scroll_top().item_ix == old_list_state.item_count() {
        new_scroll_top.item_ix += added_elements_bottom;

        if added_elements_bottom > 0 {
          new_scroll_top.offset_in_item = Pixels(0.);
        }
      }

      new_scroll_top.item_ix += shift;

      new_list_state.scroll_to(new_scroll_top);
    };

    self.list_state.update(cx, |v, _| *v = Some(new_list_state.clone()));

    new_list_state
  }

  fn update(&mut self, cx: &mut gpui::ViewContext<Self>) {
    let mut dirty = None;

    let mut flags = *self.bounds_flags.read(cx);

    // update bottom
    if flags.after {
      let cache_model = self.cache.clone();
      let list_handle = self.list.clone();

      self.cache.update(cx, |borrow, cx| {
        let last = borrow.last();

        let index = if let Some(last) = last {
          AsyncListIndex::After(if let Element::Resolved(Some(v)) = last {
            v.get_list_identifier()
          } else {
            flags.after = false;
            return;
          })
        } else {
          AsyncListIndex::RelativeToBottom(0)
        };

        borrow.push(Element::Unresolved);

        let insert_index = borrow.len() - 1;
        let mut async_ctx = cx.to_async();

        cx.foreground_executor()
          .spawn(async move {
            let (sender, receiver) = catty::oneshot();

            tokio::spawn(async move {
              sender.send(list_handle.read().await.get(index).await).unwrap();
            });

            let v = receiver.await.unwrap();

            cache_model
              .update(&mut async_ctx, |borrow, cx| {
                borrow[insert_index] = Element::Resolved(v.map(|v| v.content));

                cx.notify();
              })
              .unwrap();
          })
          .detach();

        dirty = Some(ListStateDirtyState { new_items: 1, shift: 0 });
      });
    }

    // update top
    if flags.before {
      let cache_model = self.cache.clone();
      let list_handle = self.list.clone();

      self.cache.update(cx, |borrow, cx| {
        let first = borrow.first();

        let index = if let Some(first) = first {
          AsyncListIndex::Before(if let Element::Resolved(Some(v)) = first {
            v.get_list_identifier()
          } else {
            flags.before = false;
            return;
          })
        } else {
          flags.before = false;
          return;
        };

        borrow.insert(0, Element::Unresolved);

        let insert_index = 0;
        let mut async_ctx = cx.to_async();

        cx.foreground_executor()
          .spawn(async move {
            let (sender, receiver) = catty::oneshot();

            tokio::spawn(async move {
              sender.send(list_handle.read().await.get(index).await).unwrap();
            });

            let v = receiver.await.unwrap();

            cache_model
              .update(&mut async_ctx, |borrow, cx| {
                borrow[insert_index] = Element::Resolved(v.map(|v| v.content));
                cx.notify();
              })
              .unwrap();
          })
          .detach();

        dirty = {
          let mut v = dirty.unwrap_or(ListStateDirtyState { new_items: 0, shift: 0 });

          v.shift += 1;

          Some(v)
        };
      });
    }

    self.list_state_dirty.update(cx, |v, _| {
      *v = dirty;
    });

    if dirty.is_some() {
      cx.notify();
    }

    self.bounds_flags.update(cx, |v, _| {
      if flags.after {
        v.after = false;
      }

      if flags.before {
        v.before = false;
      }
    })
  }
}

impl<T: Channel + 'static> Render for MessageListComponent<T> {
  fn render(&mut self, cx: &mut gpui::ViewContext<Self>) -> impl gpui::IntoElement {
    self.update(cx);

    let ls = if let Some(v) = self.list_state.read(cx).clone() {
      v
    } else {
      let list_state = self.list_state(cx);

      let lsc = list_state.clone();

      self.list_state.update(cx, move |v, _| *v = Some(lsc));

      list_state
    };

    div().w_full().h_full().child(list(ls).w_full().h_full())
  }
}
