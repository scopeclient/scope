use gpui::*;

#[derive(Clone)]
pub struct State {
  pub token: Option<String>,
}

#[derive(Clone)]
pub struct StateModel {
  pub inner: Model<State>,
}

impl StateModel {
  pub fn init(cx: &mut AppContext) {
    let model = cx.new_model(|_cx| State { token: None });
    let this = Self { inner: model };
    cx.set_global(this.clone());
  }

  pub fn update(f: impl FnOnce(&mut Self, &mut AppContext), cx: &mut AppContext) {
    if !cx.has_global::<Self>() {
      return;
    }
    cx.update_global::<Self, _>(|mut this, cx| {
      f(&mut this, cx);
    });
  }

  pub fn provide_token(&self, token: String, cx: &mut AppContext) {
    self.inner.update(cx, |model, _| model.token = Some(token))
  }

  pub fn take_token(&self, cx: &mut AppContext) -> Option<String> {
    self.inner.update(cx, |model, _| model.token.take())
  }
}

impl Global for StateModel {}

#[derive(Clone, Debug)]
pub struct ListChangedEvent {}

impl EventEmitter<ListChangedEvent> for State {}
