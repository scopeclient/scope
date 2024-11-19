pub mod element;
pub mod marker;

use std::{cell::RefCell, rc::Rc};

use element::{AsyncListComponentElement, AsyncListComponentElementView};
use gpui::{
  div, list, rgb, AnyElement, AppContext, Context, IntoElement, ListAlignment, ListOffset, ListState, Model, ParentElement, Pixels, Render, Styled,
  View, VisualContext,
};
use marker::Marker;
use scope_chat::async_list::{AsyncList, AsyncListIndex, AsyncListItem};

#[derive(Clone, Copy)]
struct ListStateDirtyState {
  pub new_items: usize,
}

struct BoundFlags {
  pub before: bool,
  pub after: bool,
}

pub struct AsyncListComponent<T: AsyncList>
where
  T::Content: 'static,
{
  list: Rc<RefCell<T>>,
  cache: Rc<RefCell<Vec<View<AsyncListComponentElementView<T::Content>>>>>,
  overdraw: Pixels,

  // top, bottom
  bounds_flags: Model<BoundFlags>,

  renderer: Rc<RefCell<dyn Fn(&T::Content) -> AnyElement>>,

  list_state: Model<Option<ListState>>,
  list_state_dirty: Model<Option<ListStateDirtyState>>,
}

pub enum StartAt {
  Bottom,
  Top,
}

impl<T: AsyncList> AsyncListComponent<T>
where
  T: 'static,
{
  pub fn create(cx: &mut AppContext, list: T, overdraw: Pixels, renderer: impl (Fn(&T::Content) -> AnyElement) + 'static) -> Self {
    AsyncListComponent {
      list: Rc::new(RefCell::new(list)),
      cache: Default::default(),
      overdraw,
      bounds_flags: cx.new_model(|_| BoundFlags { before: false, after: false }),
      renderer: Rc::new(RefCell::new(renderer)),
      list_state: cx.new_model(|_| None),
      list_state_dirty: cx.new_model(|_| None),
    }
  }

  fn list_state(&self, _: &mut gpui::ViewContext<Self>) -> ListState {
    let handle = self.cache.clone();
    let len = self.cache.borrow().len();
    let bounds_model = self.bounds_flags.clone();

    ListState::new(
      if len == 0 { 1 } else { len + 2 },
      ListAlignment::Bottom,
      self.overdraw,
      move |idx, cx| {
        if len == 0 {
          cx.update_model(&bounds_model, |v, _| v.after = true);

          return div().child(cx.new_view(|_| Marker { name: "Empty" })).into_any_element();
        }

        if idx == 0 {
          cx.update_model(&bounds_model, |v, _| v.before = true);

          div().child(cx.new_view(|_| Marker { name: "Upper" }))
        } else if idx == len + 1 {
          cx.update_model(&bounds_model, |v, _| v.after = true);

          div().child(cx.new_view(|_| Marker { name: "Lower" }))
        } else {
          div().text_color(rgb(0xFFFFFF)).child(handle.borrow().get(idx - 1).unwrap().clone())
        }
        .into_any_element()
      },
    )
  }

  fn get_or_refresh_list_state(&self, cx: &mut gpui::ViewContext<Self>) -> ListState {
    let list_state_dirty = self.list_state_dirty.read(cx).clone();

    if list_state_dirty.is_none() {
      if let Some(list_state) = self.list_state.read(cx) {
        return list_state.clone();
      }
    }

    let new_list_state = self.list_state(cx);
    let old_list_state = self.list_state.read(cx);

    if let Some(old_list_state) = old_list_state {
      let mut new_scroll_top = old_list_state.logical_scroll_top();

      if let Some(list_state_dirty) = list_state_dirty {
        new_scroll_top.item_ix += list_state_dirty.new_items;
      }

      new_list_state.scroll_to(new_scroll_top);
    };

    self.list_state.update(cx, |v, _| *v = Some(new_list_state.clone()));

    new_list_state
  }

  fn update(&mut self, cx: &mut gpui::ViewContext<Self>) {
    let mut dirty = None;

    // update bottom
    'update_bottom: {
      if self.bounds_flags.read(cx).after {
        let mut borrow = self.cache.borrow_mut();
        let last = borrow.last();

        let index = if let Some(last) = last {
          AsyncListIndex::After(if let AsyncListComponentElement::Resolved(v) = last.model.read(cx).element.read(cx) {
            v.get_list_identifier()
          } else {
            break 'update_bottom;
          })
        } else {
          AsyncListIndex::RelativeToBottom(0)
        };

        let list = self.list.clone();

        let renderer = self.renderer.clone();

        borrow.push(cx.new_view(move |cx| {
          AsyncListComponentElementView::new(cx, move |rf| (renderer.borrow())(rf), async move { list.borrow_mut().get(index).await })
        }));

        cx.on_next_frame(|_, cx| cx.notify());

        dirty = Some(ListStateDirtyState { new_items: 1 });
      }
    }

    // update top
    'update_top: {
      if self.bounds_flags.read(cx).before {
        let mut borrow = self.cache.borrow_mut();
        let first = borrow.first();

        let index = if let Some(first) = first {
          AsyncListIndex::Before(if let AsyncListComponentElement::Resolved(v) = first.model.read(cx).element.read(cx) {
            v.get_list_identifier()
          } else {
            break 'update_top;
          })
        } else {
          break 'update_top;
        };

        let list = self.list.clone();

        let renderer = self.renderer.clone();

        println!("Inserting at top, aka {:?}", index);

        borrow.insert(
          0,
          cx.new_view(move |cx| {
            AsyncListComponentElementView::new(cx, move |rf| (renderer.borrow())(rf), async move {
              let result = list.borrow_mut().get(index.clone()).await;
              println!("{:?} resolved to {:?}", index, result);

              result
            })
          }),
        );

        cx.on_next_frame(|_, cx| cx.notify());

        dirty = dirty.or(Some(ListStateDirtyState { new_items: 0 }));
      }
    }

    if dirty.is_some() {
      self.list_state_dirty.update(cx, |v, _| *v = dirty);
    }

    self.bounds_flags.update(cx, |v, _| {
      v.after = false;
      v.before = false;
    })
  }
}

impl<T: AsyncList + 'static> Render for AsyncListComponent<T> {
  fn render(&mut self, cx: &mut gpui::ViewContext<Self>) -> impl gpui::IntoElement {
    self.update(cx);

    div().w_full().h_full().child(list(self.get_or_refresh_list_state(cx)).w_full().h_full())
  }
}
