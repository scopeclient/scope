pub mod refcache;
pub mod refcacheslice;

use std::{collections::HashMap, process::id};

use refcache::CacheReferences;
use scope_chat::async_list::{AsyncList, AsyncListIndex, AsyncListItem, AsyncListResult};
use tokio::sync::RwLock;

pub struct AsyncListCache<L: AsyncList> {
  underlying: L,
  cache_refs: RwLock<CacheReferences<<L::Content as AsyncListItem>::Identifier>>,
  cache_map: RwLock<HashMap<<L::Content as AsyncListItem>::Identifier, L::Content>>,
}

impl<L: AsyncList> AsyncListCache<L> {}

impl<L: AsyncList> AsyncList for AsyncListCache<L> {
  type Content = L::Content;

  async fn bounded_at_top_by(&self) -> Option<<Self::Content as AsyncListItem>::Identifier> {
    let refs_read_bound = self.cache_refs.read().await;
    refs_read_bound.top_bound().await
  }

  async fn bounded_at_bottom_by(&self) -> Option<<Self::Content as AsyncListItem>::Identifier> {
    let refs_read_bound = self.cache_refs.read().await;
    refs_read_bound.bottom_bound().await
  }

  async fn get(&self, index: AsyncListIndex<<Self::Content as AsyncListItem>::Identifier>) -> Option<AsyncListResult<L::Content>> {
    let cache_read_handle = self.cache_refs.read().await;
    let cache_result = cache_read_handle.get(index.clone()).await;

    if let Some(cache_result) = cache_result {
      let content = self.cache_map.read().await.get(&cache_result).unwrap().clone();
      let is_first = cache_read_handle.top_bound().await.map(|v| v == content.get_list_identifier()).unwrap_or(false);
      let is_last = cache_read_handle.bottom_bound().await.map(|v| v == content.get_list_identifier()).unwrap_or(false);

      return Some(AsyncListResult { content, is_first, is_last });
    };

    let authoritative = self.underlying.get(index.clone()).await;

    if let Some(ref authoritative) = authoritative {
      let identifier = authoritative.content.get_list_identifier();

      self.cache_map.write().await.insert(identifier.clone(), authoritative.content.clone());
      self.cache_refs.write().await.insert(index, identifier.clone(), authoritative.is_first, authoritative.is_last).await;
    }

    authoritative
  }

  async fn find(&self, identifier: &<L::Content as AsyncListItem>::Identifier) -> Option<AsyncListResult<L::Content>> {
    let cache_read_handle = self.cache_map.read().await;
    let cache_result = cache_read_handle.get(identifier);

    if let Some(cache_result) = cache_result {
      let content = cache_result.clone();
      let is_first = self.bounded_at_top_by().await.map(|v| v == *identifier).unwrap_or(false);
      let is_last = self.bounded_at_bottom_by().await.map(|v| v == *identifier).unwrap_or(false);

      return Some(AsyncListResult { content, is_first, is_last });
    };

    let authoritative = self.underlying.find(identifier).await;

    if let Some(ref authoritative) = authoritative {
      self.cache_map.write().await.insert(identifier.clone(), authoritative.content.clone());
    }

    authoritative
  }
}
