use std::{fmt::Debug, future::Future, hash::Hash};

pub trait AsyncList {
  type Content: AsyncListItem;

  fn bounded_at_top_by(&self) -> impl Future<Output = Option<<Self::Content as AsyncListItem>::Identifier>>;
  fn get(
    &self,
    index: AsyncListIndex<<Self::Content as AsyncListItem>::Identifier>,
  ) -> impl Future<Output = Option<AsyncListResult<Self::Content>>> + Send;
  fn find(&self, identifier: &<Self::Content as AsyncListItem>::Identifier) -> impl Future<Output = Option<Self::Content>>;
  fn bounded_at_bottom_by(&self) -> impl Future<Output = Option<<Self::Content as AsyncListItem>::Identifier>>;
}

pub trait AsyncListItem: Clone + Debug {
  type Identifier: Eq + Hash + Clone + Send + Debug;

  fn get_list_identifier(&self) -> Self::Identifier;
}

#[derive(Clone)]
pub enum AsyncListIndex<I: Clone> {
  RelativeToTop(usize),
  /// Before is closer to the top
  Before(I),

  RelativeToBottom(usize),
  /// After is closer to the bottom
  After(I),
}

impl<I: Clone + Copy> Copy for AsyncListIndex<I> {}

impl<I: Clone + Debug> Debug for AsyncListIndex<I> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::After(i) => f.debug_tuple("AsyncListIndex::After").field(i).finish()?,
      Self::Before(i) => f.debug_tuple("AsyncListIndex::Before").field(i).finish()?,
      Self::RelativeToTop(i) => f.debug_tuple("AsyncListIndex::RelativeToTop").field(i).finish()?,
      Self::RelativeToBottom(i) => f.debug_tuple("AsyncListIndex::RelativeToBottom").field(i).finish()?,
    };

    Ok(())
  }
}

#[derive(Clone)]
pub struct AsyncListResult<T> {
  pub content: T,
  pub is_top: bool,
  pub is_bottom: bool,
}

impl<I: Clone + Debug> Debug for AsyncListResult<I> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("AsyncListResult").field("content", &self.content).field("is_top", &self.is_top).field("is_bottom", &self.is_bottom).finish()
  }
}
