use std::collections::HashMap;

use scope_chat::async_list::AsyncListIndex;

struct CacheReferencesSlice<I: Clone> {
  is_bounded_at_top: bool,
  is_bounded_at_bottom: bool,

  // the vec's 0th item is the top, and it's last item is the bottom
  // the vec MUST NOT be empty.
  item_references: Vec<I>,
}

pub struct CacheReferences<I: Clone> {
  // dense segments are unordered (spooky!) slices of content we do! know about.
  // the u64 in the hashmap represents a kind of "segment identifier"
  dense_segment: HashMap<u64, CacheReferencesSlice<I>>,

  top_bounded_identifier: Option<u64>,
  bottom_bounded_identifier: Option<u64>,
}

impl<I: Clone> CacheReferences<I> {
  pub fn top_bound(&self) -> Option<&I> {
    self.top_bounded_identifier.map(|v| {
      let top_bound = self.dense_segment.get(&v).unwrap();

      assert!(top_bound.is_bounded_at_top);

      top_bound.item_references.first().unwrap()
    })
  }

  pub fn bottom_bound(&self) -> Option<&I> {
    self.bottom_bounded_identifier.map(|v| {
      let bottom_bound = self.dense_segment.get(&v).unwrap();

      assert!(bottom_bound.is_bounded_at_bottom);

      bottom_bound.item_references.last().unwrap()
    })
  }

  pub fn get(&self, index: AsyncListIndex<I>) -> Option<I> {
    unimplemented!()
  }
}
