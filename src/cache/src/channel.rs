use std::sync::Arc;

use scope_chat::{
  async_list::{AsyncList, AsyncListIndex, AsyncListItem, AsyncListResult},
  channel::Channel,
};
use tokio::sync::Mutex;

use crate::async_list::{refcacheslice::Exists, AsyncListCache};

pub struct CacheChannel<T: Channel>(T, Arc<Mutex<AsyncListCache<T>>>);

impl<T: Channel> CacheChannel<T> {
  pub fn new(channel: T) -> Self
  where
    T::Message: 'static,
    T: 'static,
  {
    let mut receiver = channel.get_receiver();
    let cache = Arc::new(Mutex::new(AsyncListCache::new()));
    let cache_clone = cache.clone();

    tokio::spawn(async move {
      loop {
        let message = receiver.recv().await.unwrap();

        cache_clone.lock().await.append_bottom(message);
      }
    });

    CacheChannel(channel, cache)
  }
}

impl<T: Channel> Channel for CacheChannel<T> {
  type Message = T::Message;

  fn get_receiver(&self) -> tokio::sync::broadcast::Receiver<Self::Message> {
    self.0.get_receiver()
  }

  fn send_message(&self, content: String, nonce: String) -> Self::Message {
    self.0.send_message(content, nonce)
  }
}

impl<T: Channel> AsyncList for CacheChannel<T> {
  type Content = T::Message;

  async fn bounded_at_top_by(&self) -> Option<<Self::Content as AsyncListItem>::Identifier> {
    let l = self.1.lock().await;
    let v = l.bounded_at_top_by();

    if let Some(v) = v {
      return Some(v);
    };

    let i = self.0.bounded_at_top_by().await?;

    Some(i)
  }

  async fn bounded_at_bottom_by(&self) -> Option<<Self::Content as AsyncListItem>::Identifier> {
    let l = self.1.lock().await;
    let v = l.bounded_at_bottom_by();

    if let Some(v) = v {
      return Some(v);
    };

    let i = self.0.bounded_at_bottom_by().await?;

    Some(i)
  }

  async fn get(&self, index: AsyncListIndex<<Self::Content as AsyncListItem>::Identifier>) -> Option<AsyncListResult<Self::Content>> {
    let mut l = self.1.lock().await;
    let v = l.get(index.clone());

    if let Exists::Yes(v) = v {
      return Some(v);
    } else if let Exists::No = v {
      return None;
    }

    let authoritative = self.0.get(index.clone()).await?;

    l.insert(index, authoritative.content.clone(), authoritative.is_top, authoritative.is_bottom);

    Some(authoritative)
  }

  async fn find(&self, identifier: &<Self::Content as AsyncListItem>::Identifier) -> Option<Self::Content> {
    let mut l = self.1.lock().await;
    let v = l.find(identifier);

    if let Some(v) = v {
      return Some(v);
    }

    let authoritative = self.0.find(identifier).await?;

    l.insert_unlocated(authoritative.clone());

    Some(authoritative)
  }
}
