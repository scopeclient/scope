use std::collections::HashMap;

use scope_chat::async_list::AsyncListIndex;
use tokio::{fs::read, sync::RwLock};

use super::refcacheslice::{self, CacheReferencesSlice};

pub struct CacheReferences<I: Clone + Eq + PartialEq> {
  // dense segments are unordered (spooky!) slices of content we do! know about.
  // the u64 in the hashmap represents a kind of "segment identifier"
  dense_segments: RwLock<HashMap<u64, CacheReferencesSlice<I>>>,

  top_bounded_identifier: Option<u64>,
  bottom_bounded_identifier: Option<u64>,
}

impl<I: Clone + Eq + PartialEq> CacheReferences<I> {
  pub async fn top_bound(&self) -> Option<I> {
    let index = self.top_bounded_identifier?;
    let read_handle = self.dense_segments.read().await;
    let top_bound = read_handle.get(&index).unwrap();

    assert!(top_bound.is_bounded_at_top);

    Some(top_bound.item_references.first().unwrap().clone())
  }

  pub async fn bottom_bound(&self) -> Option<I> {
    let index = self.bottom_bounded_identifier?;
    let read_handle = self.dense_segments.read().await;
    let bottom_bound = read_handle.get(&index).unwrap();

    assert!(bottom_bound.is_bounded_at_bottom);

    Some(bottom_bound.item_references.last().unwrap().clone())
  }

  pub async fn get(&self, index: AsyncListIndex<I>) -> Option<I> {
    let read_handle = self.dense_segments.read().await;
    
    for segment in read_handle.values() {
      if let Some(value) = segment.get(index.clone()) {
        return Some(value)
      }
    }

    return None;
  }

  /// you mut **KNOW** that the item you are inserting is not:
  ///  - directly next to (Before or After) **any** item in the list
  ///  - the first or last item in the list
  pub async fn insert_detached(&self, item: I) {
    let mut mutation_handle = self.dense_segments.write().await;

    mutation_handle.insert(rand::random(), CacheReferencesSlice {
      is_bounded_at_top: false,
      is_bounded_at_bottom: false,

      item_references: vec![item],
    });
  }

  pub async fn insert(&self, index: AsyncListIndex<I>, item: I,) {
    // insert routine is really complex:
    // an insert can "join" together 2 segments
    // an insert can append to a segment
    // or an insert can construct a new segment

    let mut segments = vec![];

    let read_handle = self.dense_segments.read().await;

    for (i, segment) in read_handle.iter() {
      if let Some(position) = segment.can_insert(index.clone()) {
        segments.push((position, i));
      }
    }

    if segments.len() == 0 {
      let mut mutation_handle = self.dense_segments.write().await;

      mutation_handle.insert(rand::random(), CacheReferencesSlice {
        is_bounded_at_top: is_first,
        is_bounded_at_bottom: is_last,

        item_references: vec![item],
      });
    } else if segments.len() == 1 {
      let mut mutation_handle = self.dense_segments.write().await;

      mutation_handle.get_mut(segments[0].1).unwrap().insert(index.clone(), item);
    } else if segments.len() == 2 {
      let (li, ri) = match (segments[0], segments[1]) {
        ((refcacheslice::Position::After, lp), (refcacheslice::Position::Before, rp)) => (lp, rp),
        ((refcacheslice::Position::Before, rp), (refcacheslice::Position::After, lp)) => (lp, rp),

        _ => panic!("How are there two candidates that aren't (Before, After) or (After, Before)?")
      };

      let mut mutation_handle = self.dense_segments.write().await;

      let (left, right) = if li < ri {
        let right = mutation_handle.remove(&ri).unwrap();
        let left = mutation_handle.remove(&li).unwrap();

        (left, right)
      } else {
        let left = mutation_handle.remove(&li).unwrap();
        let right = mutation_handle.remove(&ri).unwrap();

        (left, right)
      };

      let mut merged = left.item_references;
      
      merged.push(item);

      merged.extend(right.item_references.into_iter());

      mutation_handle.insert(rand::random(), CacheReferencesSlice {
        is_bounded_at_top: left.is_bounded_at_top,
        is_bounded_at_bottom: right.is_bounded_at_bottom,

        item_references: merged,
      });
    } else {
      panic!("Impossible state")
    }
  }
}
