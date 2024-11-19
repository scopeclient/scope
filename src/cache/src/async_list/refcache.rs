use std::{collections::HashMap, fmt::Debug};

use scope_chat::async_list::AsyncListIndex;

use super::refcacheslice::{self, CacheReferencesSlice, Exists};

pub struct CacheReferences<I: Clone + Eq + PartialEq> {
  // dense segments are unordered (spooky!) slices of content we do! know about.
  // the u64 in the hashmap represents a kind of "segment identifier"
  dense_segments: HashMap<u64, CacheReferencesSlice<I>>,

  top_bounded_identifier: Option<u64>,
  bottom_bounded_identifier: Option<u64>,
}

impl<I: Clone + Eq + PartialEq> CacheReferences<I> {
  pub fn new() -> Self {
    Self {
      dense_segments: HashMap::new(),
      top_bounded_identifier: None,
      bottom_bounded_identifier: None,
    }
  }

  pub fn append_bottom(&mut self, identifier: I) {
    let mut id = None;

    for (segment_id, segment) in self.dense_segments.iter() {
      if let Exists::Yes(_) = segment.get(AsyncListIndex::RelativeToBottom(0)) {
        if id.is_some() {
          panic!("There should only be one bottom bound segment");
        }

        id = Some(*segment_id)
      }
    }

    if let Some(id) = id {
      self.dense_segments.get_mut(&id).unwrap().append_bottom(identifier);
    } else {
      self.insert(AsyncListIndex::RelativeToBottom(0), identifier, false, true);
    }
  }

  pub fn top_bound(&self) -> Option<I> {
    let index = self.top_bounded_identifier?;
    let top_bound = self.dense_segments.get(&index).unwrap();

    assert!(top_bound.is_bounded_at_top);

    Some(top_bound.item_references.first().unwrap().clone())
  }

  pub fn bottom_bound(&self) -> Option<I> {
    let index = self.bottom_bounded_identifier?;
    let bottom_bound = self.dense_segments.get(&index).unwrap();

    assert!(bottom_bound.is_bounded_at_bottom);

    Some(bottom_bound.item_references.last().unwrap().clone())
  }

  pub fn get(&self, index: AsyncListIndex<I>) -> Exists<I> {
    for segment in self.dense_segments.values() {
      let result = segment.get(index.clone());

      if let Exists::Yes(value) = result {
        return Exists::Yes(value);
      } else if let Exists::No = result {
        return Exists::No;
      }
    }

    return Exists::Unknown;
  }

  /// you mut **KNOW** that the item you are inserting is not:
  ///  - directly next to (Before or After) **any** item in the list
  ///  - the first or last item in the list
  pub fn insert_detached(&mut self, item: I) {
    self.dense_segments.insert(
      rand::random(),
      CacheReferencesSlice {
        is_bounded_at_top: false,
        is_bounded_at_bottom: false,

        item_references: vec![item],
      },
    );
  }

  pub fn insert(&mut self, index: AsyncListIndex<I>, item: I, is_top: bool, is_bottom: bool) {
    // insert routine is really complex:
    // an insert can "join" together 2 segments
    // an insert can append to a segment
    // or an insert can construct a new segment

    let mut segments = vec![];

    for (i, segment) in self.dense_segments.iter() {
      if let Some(position) = segment.can_insert(index.clone()) {
        segments.push((position, *i));
      }
    }

    if segments.len() == 0 {
      let id = rand::random();

      self.dense_segments.insert(
        id,
        CacheReferencesSlice {
          is_bounded_at_top: is_top,
          is_bounded_at_bottom: is_bottom,

          item_references: vec![item],
        },
      );

      if is_bottom {
        self.bottom_bounded_identifier = Some(id);
      }

      if is_top {
        self.top_bounded_identifier = Some(id);
      }
    } else if segments.len() == 1 {
      self.dense_segments.get_mut(&segments[0].1).unwrap().insert(index.clone(), item, is_bottom, is_top);

      if is_top {
        self.top_bounded_identifier = Some(segments[0].1)
      }
      if is_bottom {
        self.bottom_bounded_identifier = Some(segments[0].1)
      }
    } else if segments.len() == 2 {
      assert!(!is_top);
      assert!(!is_bottom);

      let (li, ri) = match (segments[0], segments[1]) {
        ((refcacheslice::Position::After, lp), (refcacheslice::Position::Before, rp)) => (lp, rp),
        ((refcacheslice::Position::Before, rp), (refcacheslice::Position::After, lp)) => (lp, rp),

        _ => panic!("How are there two candidates that aren't (Before, After) or (After, Before)?"),
      };

      let (left, right) = if li < ri {
        let right = self.dense_segments.remove(&ri).unwrap();
        let left = self.dense_segments.remove(&li).unwrap();

        (left, right)
      } else {
        let left = self.dense_segments.remove(&li).unwrap();
        let right = self.dense_segments.remove(&ri).unwrap();

        (left, right)
      };

      let mut merged = left.item_references;

      merged.push(item);

      merged.extend(right.item_references.into_iter());

      let id = rand::random();

      self.dense_segments.insert(
        id,
        CacheReferencesSlice {
          is_bounded_at_top: left.is_bounded_at_top,
          is_bounded_at_bottom: right.is_bounded_at_bottom,

          item_references: merged,
        },
      );

      if left.is_bounded_at_top {
        self.top_bounded_identifier = Some(id);
      }

      if right.is_bounded_at_bottom {
        self.bottom_bounded_identifier = Some(id);
      }
    } else {
      panic!("Impossible state")
    }
  }
}

impl<I: Clone + Eq + PartialEq + Debug> Debug for CacheReferences<I> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("CacheReferences")
      .field("top_bounded_segment", &self.top_bounded_identifier)
      .field("bottom_bounded_segment", &self.bottom_bounded_identifier)
      .field("dense_segments", &self.dense_segments)
      .finish()
  }
}
