pub mod refcache;

use std::{collections::HashMap, future::Future};

use refcache::CacheReferences;
use scope_chat::{
  async_list::{AsyncList, AsyncListIndex, AsyncListItem},
  channel::Channel,
};
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
    refs_read_bound.top_bound().cloned()
  }

  async fn bounded_at_bottom_by(&self) -> Option<<Self::Content as AsyncListItem>::Identifier> {
    let refs_read_bound = self.cache_refs.read().await;
    refs_read_bound.bottom_bound().cloned()
  }

  async fn get(&self, index: AsyncListIndex<<Self::Content as AsyncListItem>::Identifier>) -> Option<L::Content> {
    let cache_read_handle = self.cache_refs.read().await;
    let cache_result = cache_read_handle.get(index.clone());

    if let Some(cache_result) = cache_result {
      return Some(self.cache_map.read().await.get(&cache_result).unwrap().clone());
    };

    let authoritative = self.underlying.get(index).await;

    if let Some(ref authoritative) = authoritative {
      unimplemented!()
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
