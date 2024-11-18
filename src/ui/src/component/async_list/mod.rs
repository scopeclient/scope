pub mod element;
pub mod marker;

use std::{cell::RefCell, rc::Rc};

use element::{AsyncListComponentElement, AsyncListComponentElementView};
use gpui::{
  div, list, rgb, AnyElement, AppContext, Context, Element, IntoElement, ListAlignment, ListState, Model, ParentElement, Pixels, Render, Styled,
  View, VisualContext,
};
use marker::Marker;
use scope_chat::async_list::{AsyncList, AsyncListIndex, AsyncListItem};

pub struct AsyncListComponent<T: AsyncList>
where
  T::Content: 'static,
{
  list: Rc<RefCell<T>>,
  cache: Rc<RefCell<Vec<View<AsyncListComponentElementView<T::Content>>>>>,
  alignment: ListAlignment,
  overdraw: Pixels,

  // top, bottom
  bounds_flags: Model<(bool, bool)>,

  renderer: Rc<RefCell<dyn Fn(&T::Content) -> AnyElement>>,
}

pub enum StartAt {
  Bottom,
  Top,
}

impl<T: AsyncList> AsyncListComponent<T>
where
  T: 'static,
{
  pub fn create(cx: &mut AppContext, list: T, start_at: StartAt, overdraw: Pixels, renderer: impl (Fn(&T::Content) -> AnyElement) + 'static) -> Self {
    AsyncListComponent {
      list: Rc::new(RefCell::new(list)),
      cache: Default::default(),
      alignment: if let StartAt::Bottom = start_at {
        ListAlignment::Bottom
      } else {
        ListAlignment::Top
      },
      overdraw,
      bounds_flags: cx.new_model(|_| (false, false)),
      renderer: Rc::new(RefCell::new(renderer)),
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
        if idx == 0 {
          cx.update_model(&bounds_model, |v, _| *v = (true, v.1));

          div().child(cx.new_view(|_| Marker { name: "Upper" }))
        } else if idx == len + 1 {
          cx.update_model(&bounds_model, |v, _| *v = (v.0, true));

          div().child(cx.new_view(|_| Marker { name: "Lower" }))
        } else {
          div().text_color(rgb(0xFFFFFF)).child(handle.borrow().get(idx - 1).unwrap().clone())
        }
        .into_any_element()
      },
    )
  }

  fn update(&mut self, cx: &mut gpui::ViewContext<Self>) {
    // update bottom
    'update_bottom: {
      if self.bounds_flags.read(cx).0 {
        println!("Updating Bottom!");
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

        println!("  {:?}", index);

        let list = self.list.clone();

        println!("Pushed to bottom");

        let renderer = self.renderer.clone();

        let len = borrow.len();

        borrow.push(cx.new_view(move |cx| {
          AsyncListComponentElementView::new(cx, move |rf| (renderer.borrow())(rf), async move { list.borrow_mut().get(index).await })
        }));

        cx.on_next_frame(|v, cx| cx.notify());
      }
    }

    // update top
    'update_top: {
      if self.bounds_flags.read(cx).1 {
        let mut borrow = self.cache.borrow_mut();
        let first = borrow.first();

        let index = if let Some(first) = first {
          AsyncListIndex::Before(if let AsyncListComponentElement::Resolved(v) = first.model.read(cx).element.read(cx) {
            v.get_list_identifier()
          } else {
            break 'update_top;
          })
        } else {
          AsyncListIndex::RelativeToTop(0)
        };

        let list = self.list.clone();

        println!("Pushed to top");

        let renderer = self.renderer.clone();

        borrow.insert(
          0,
          cx.new_view(move |cx| {
            AsyncListComponentElementView::new(cx, move |rf| (renderer.borrow())(rf), async move { list.borrow_mut().get(index).await })
          }),
        );

        cx.on_next_frame(|v, cx| cx.notify());
      }
    }

    self.bounds_flags.update(cx, |v, _| *v = (false, false))
  }
}

impl<T: AsyncList + 'static> Render for AsyncListComponent<T> {
  fn render(&mut self, cx: &mut gpui::ViewContext<Self>) -> impl gpui::IntoElement {
    println!("Rendering");

    self.update(cx);

    div().w_full().h_full().child(list(self.list_state(cx)).w_full().h_full())
  }
}
