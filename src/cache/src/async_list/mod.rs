pub mod refcache;
pub mod refcacheslice;

use std::collections::HashMap;

use refcache::CacheReferences;
use refcacheslice::Exists;
use scope_chat::async_list::{AsyncList, AsyncListIndex, AsyncListItem, AsyncListResult};

pub struct AsyncListCache<L: AsyncList> {
  cache_refs: CacheReferences<<L::Content as AsyncListItem>::Identifier>,
  cache_map: HashMap<<L::Content as AsyncListItem>::Identifier, L::Content>,
}

impl<L: AsyncList> AsyncListCache<L> {
  pub fn new() -> Self {
    Self {
      cache_refs: CacheReferences::new(),
      cache_map: HashMap::new(),
    }
  }

  pub fn append_bottom(&mut self, value: L::Content) {
    let identifier = value.get_list_identifier();

    self.cache_refs.append_bottom(identifier.clone());
    self.cache_map.insert(identifier, value);
  }

  pub fn insert(&mut self, index: AsyncListIndex<<L::Content as AsyncListItem>::Identifier>, value: L::Content, is_top: bool, is_bottom: bool) {
    let identifier = value.get_list_identifier();

    self.cache_map.insert(identifier.clone(), value);
    self.cache_refs.insert(index, identifier.clone(), is_top, is_bottom);
  }

  pub fn insert_unlocated(&mut self, value: L::Content) {
    let identifier = value.get_list_identifier();

    self.cache_map.insert(identifier.clone(), value);
  }

  pub fn bounded_at_top_by(&self) -> Option<<L::Content as AsyncListItem>::Identifier> {
    self.cache_refs.top_bound()
  }

  pub fn bounded_at_bottom_by(&self) -> Option<<L::Content as AsyncListItem>::Identifier> {
    self.cache_refs.bottom_bound()
  }

  pub fn get(&self, index: AsyncListIndex<<L::Content as AsyncListItem>::Identifier>) -> Exists<AsyncListResult<L::Content>> {
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

  pub fn find(&self, identifier: &<L::Content as AsyncListItem>::Identifier) -> Option<L::Content> {
    self.cache_map.get(identifier).cloned()
  }
}
