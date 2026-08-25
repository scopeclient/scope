pub mod refcache;
pub mod refcacheslice;
pub mod tests;

use std::collections::HashMap;

use refcache::CacheReferences;
use refcacheslice::Exists;
use scope_chat::async_list::{AsyncListIndex, AsyncListItem, AsyncListResult};

pub struct AsyncListCache<I: AsyncListItem> {
  cache_refs: CacheReferences<I::Identifier>,
  cache_map: HashMap<I::Identifier, I>,
}

impl<I: AsyncListItem> Default for AsyncListCache<I> {
  fn default() -> Self {
    Self::new()
  }
}

impl<I: AsyncListItem> AsyncListCache<I> {
  pub fn new() -> Self {
    Self {
      cache_refs: CacheReferences::new(),
      cache_map: HashMap::new(),
    }
  }

  pub fn append_bottom(&mut self, value: I) {
    let identifier = value.get_list_identifier();

    self.cache_refs.append_bottom(identifier.clone());
    self.cache_map.insert(identifier, value);
  }

  pub fn insert(&mut self, index: AsyncListIndex<I::Identifier>, value: I, is_top: bool, is_bottom: bool) {
    let identifier = value.get_list_identifier();

    self.cache_map.insert(identifier.clone(), value);
    self.cache_refs.insert(index, identifier.clone(), is_top, is_bottom);
  }

  /// you mut **KNOW** that the item you are inserting is not:
  ///  - directly next to (Before or After) **any** item in the list
  ///  - the first or last item in the list
  pub fn insert_detached(&mut self, value: I) {
    let identifier = value.get_list_identifier();

    self.cache_map.insert(identifier.clone(), value);
    self.cache_refs.insert_detached(identifier);
  }

  /// Mark an already-cached item as the oldest that exists (top of history).
  pub fn mark_top_bound(&mut self, identifier: &I::Identifier) {
    self.cache_refs.mark_top_bound(identifier);
  }

  /// Mark an already-cached item as the newest that exists (bottom of history).
  pub fn mark_bottom_bound(&mut self, identifier: &I::Identifier) {
    self.cache_refs.mark_bottom_bound(identifier);
  }

  pub fn bounded_at_top_by(&self) -> Option<I::Identifier> {
    self.cache_refs.top_bound()
  }

  pub fn bounded_at_bottom_by(&self) -> Option<I::Identifier> {
    self.cache_refs.bottom_bound()
  }

  pub fn get(&self, index: AsyncListIndex<I::Identifier>) -> Exists<AsyncListResult<I>> {
    let cache_result = self.cache_refs.get(index.clone());

    if let Exists::Yes(cache_result) = cache_result {
      let content = self.cache_map.get(&cache_result).unwrap().clone();
      let is_top = self.cache_refs.top_bound().map(|v| v == content.get_list_identifier()).unwrap_or(false);
      let is_bottom = self.cache_refs.bottom_bound().map(|v| v == content.get_list_identifier()).unwrap_or(false);

      return Exists::Yes(AsyncListResult { content, is_top, is_bottom });
    };

    if let Exists::No = cache_result {
      return Exists::No;
    }

    Exists::Unknown
  }

  pub fn find(&self, identifier: &I::Identifier) -> Option<I> {
    self.cache_map.get(identifier).cloned()
  }

  /// Swap the cached item with the same identifier for `value`, keeping its position.
  /// Returns the previous item; `None` (and no change) when the item is not cached.
  pub fn replace(&mut self, value: I) -> Option<I> {
    let identifier = value.get_list_identifier();

    if !self.cache_map.contains_key(&identifier) {
      return None;
    }

    self.cache_map.insert(identifier, value)
  }

  /// Forget the item with `identifier`; its neighbours become adjacent. Returns the removed item.
  pub fn remove(&mut self, identifier: &I::Identifier) -> Option<I> {
    self.cache_refs.remove(identifier);
    self.cache_map.remove(identifier)
  }
}
