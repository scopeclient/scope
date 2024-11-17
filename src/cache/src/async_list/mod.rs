pub mod refcache;
pub mod refcacheslice;

use std::collections::HashMap;

use refcache::CacheReferences;
use scope_chat::async_list::{AsyncList, AsyncListIndex, AsyncListItem};
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

  async fn get(&self, index: AsyncListIndex<<Self::Content as AsyncListItem>::Identifier>) -> Option<L::Content> {
    let cache_read_handle = self.cache_refs.read().await;
    let cache_result = cache_read_handle.get(index.clone()).await;

    if let Some(cache_result) = cache_result {
      return Some(self.cache_map.read().await.get(&cache_result).unwrap().clone());
    };

    let authoritative = self.underlying.get(index.clone()).await;

    if let Some(ref authoritative) = authoritative {
      let identifier = authoritative.get_list_identifier();

      self.cache_map.write().await.insert(identifier.clone(), authoritative.clone());
      self.cache_refs.write().await.insert(index, identifier.clone());
    }

    authoritative
  }

  async fn find(&self, identifier: &<L::Content as AsyncListItem>::Identifier) -> Option<L::Content> {
    let cache_read_handle = self.cache_map.read().await;
    let cache_result = cache_read_handle.get(identifier);

    if let Some(cache_result) = cache_result {
      return Some(cache_result.clone());
    };

    let authoritative = self.underlying.find(identifier).await;

    if let Some(ref authoritative) = authoritative {
      self.cache_map.write().await.insert(identifier.clone(), authoritative.clone());
    }

    authoritative
  }
}
