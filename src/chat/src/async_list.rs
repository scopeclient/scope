use std::hash::Hash;

pub trait AsyncList {
  type Content: AsyncListItem;

  fn bounded_at_top_by(&self) -> impl std::future::Future<Output = Option<<Self::Content as AsyncListItem>::Identifier>>;
  fn get(&self, index: AsyncListIndex<<Self::Content as AsyncListItem>::Identifier>) -> impl std::future::Future<Output = Option<Self::Content>>;
  fn find(&self, identifier: &<Self::Content as AsyncListItem>::Identifier) -> impl std::future::Future<Output = Option<Self::Content>>;
  fn bounded_at_bottom_by(&self) -> impl std::future::Future<Output = Option<<Self::Content as AsyncListItem>::Identifier>>;
}

pub trait AsyncListItem: Clone {
  type Identifier: Eq + Hash + Clone;

  fn get_list_identifier(&self) -> Self::Identifier;
}

#[derive(Clone)]
pub enum AsyncListIndex<I: Clone> {
  RelativeToTop(usize),
  After(I),
  Before(I),
  RelativeToBottom(usize),
}

impl<I: Clone + Copy> Copy for AsyncListIndex<I> {}
