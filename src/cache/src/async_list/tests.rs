use std::fmt::Debug;

#[allow(unused_imports)]
use scope_chat::async_list::{AsyncListIndex, AsyncListItem, AsyncListResult};

#[allow(unused_imports)]
use crate::async_list::{AsyncListCache, refcacheslice::Exists};

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct ListItem(i64);

impl AsyncListItem for ListItem {
  type Identifier = i64;

  fn get_list_identifier(&self) -> Self::Identifier {
    self.0
  }
}

#[allow(dead_code)]
fn assert_query_exists<I: PartialEq + Eq + Debug>(result: Exists<AsyncListResult<I>>, item: I, is_top_in: bool, is_bottom_in: bool) {
  if let Exists::Yes(AsyncListResult { content, is_top, is_bottom }) = result {
    assert_eq!(content, item);
    assert_eq!(is_top, is_top_in);
    assert_eq!(is_bottom, is_bottom_in);
  } else {
    panic!("Expected eq yes")
  }
}

#[test]
pub fn cache_can_append_bottom_in_unbounded_state() {
  let mut cache = AsyncListCache::<ListItem>::new();

  cache.append_bottom(ListItem(0));

  assert_eq!(cache.bounded_at_bottom_by(), Some(0));
  assert_eq!(cache.find(&0), Some(ListItem(0)));
  assert_query_exists(cache.get(AsyncListIndex::RelativeToBottom(0)), ListItem(0), false, true);
}

#[test]
pub fn cache_can_append_bottom_many_times_successfully() {
  let mut cache = AsyncListCache::<ListItem>::new();

  cache.append_bottom(ListItem(0));

  assert_eq!(cache.bounded_at_bottom_by(), Some(0));
  assert_eq!(cache.find(&0), Some(ListItem(0)));
  assert_query_exists(cache.get(AsyncListIndex::RelativeToBottom(0)), ListItem(0), false, true);

  cache.append_bottom(ListItem(1));

  assert_eq!(cache.bounded_at_bottom_by(), Some(1));
  assert_eq!(cache.find(&1), Some(ListItem(1)));
  assert_eq!(cache.find(&0), Some(ListItem(0)));
  assert_query_exists(cache.get(AsyncListIndex::RelativeToBottom(0)), ListItem(1), false, true);
  assert_query_exists(cache.get(AsyncListIndex::RelativeToBottom(1)), ListItem(0), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(1)), ListItem(0), false, false);
  assert_query_exists(cache.get(AsyncListIndex::After(0)), ListItem(1), false, true);

  cache.append_bottom(ListItem(2));

  assert_eq!(cache.bounded_at_bottom_by(), Some(2));
  assert_eq!(cache.find(&2), Some(ListItem(2)));
  assert_eq!(cache.find(&1), Some(ListItem(1)));
  assert_eq!(cache.find(&0), Some(ListItem(0)));
  assert_query_exists(cache.get(AsyncListIndex::RelativeToBottom(0)), ListItem(2), false, true);
  assert_query_exists(cache.get(AsyncListIndex::RelativeToBottom(1)), ListItem(1), false, false);
  assert_query_exists(cache.get(AsyncListIndex::RelativeToBottom(2)), ListItem(0), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(1)), ListItem(0), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(2)), ListItem(1), false, false);
  assert_query_exists(cache.get(AsyncListIndex::After(0)), ListItem(1), false, false);
  assert_query_exists(cache.get(AsyncListIndex::After(1)), ListItem(2), false, true);
}

#[test]
pub fn cache_can_work_unlocated() {
  let mut cache = AsyncListCache::<ListItem>::new();

  cache.insert_detached(ListItem(0));
  assert_eq!(cache.find(&0), Some(ListItem(0)));

  cache.insert(AsyncListIndex::After(0), ListItem(2), false, false);

  assert_eq!(cache.find(&0), Some(ListItem(0)));
  assert_eq!(cache.find(&2), Some(ListItem(2)));
  assert_query_exists(cache.get(AsyncListIndex::After(0)), ListItem(2), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(2)), ListItem(0), false, false);

  cache.insert(AsyncListIndex::Before(0), ListItem(-2), false, false);
  assert_eq!(cache.find(&-2), Some(ListItem(-2)));
  assert_eq!(cache.find(&0), Some(ListItem(0)));
  assert_eq!(cache.find(&2), Some(ListItem(2)));
  assert_query_exists(cache.get(AsyncListIndex::After(0)), ListItem(2), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(2)), ListItem(0), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(0)), ListItem(-2), false, false);
  assert_query_exists(cache.get(AsyncListIndex::After(-2)), ListItem(0), false, false);
}

#[test]
pub fn cache_can_insert_between() {
  let mut cache = AsyncListCache::<ListItem>::new();

  cache.insert_detached(ListItem(0));
  assert_eq!(cache.find(&0), Some(ListItem(0)));

  cache.insert(AsyncListIndex::After(0), ListItem(2), false, false);

  assert_eq!(cache.find(&0), Some(ListItem(0)));
  assert_eq!(cache.find(&2), Some(ListItem(2)));
  assert_query_exists(cache.get(AsyncListIndex::After(0)), ListItem(2), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(2)), ListItem(0), false, false);

  cache.insert(AsyncListIndex::Before(0), ListItem(-2), false, false);
  assert_eq!(cache.find(&-2), Some(ListItem(-2)));
  assert_eq!(cache.find(&0), Some(ListItem(0)));
  assert_eq!(cache.find(&2), Some(ListItem(2)));
  assert_query_exists(cache.get(AsyncListIndex::After(0)), ListItem(2), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(2)), ListItem(0), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(0)), ListItem(-2), false, false);
  assert_query_exists(cache.get(AsyncListIndex::After(-2)), ListItem(0), false, false);

  cache.insert(AsyncListIndex::After(-2), ListItem(-1), false, false);
  assert_eq!(cache.find(&-2), Some(ListItem(-2)));
  assert_eq!(cache.find(&-1), Some(ListItem(-1)));
  assert_eq!(cache.find(&0), Some(ListItem(0)));
  assert_eq!(cache.find(&2), Some(ListItem(2)));
  assert_query_exists(cache.get(AsyncListIndex::After(0)), ListItem(2), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(2)), ListItem(0), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(0)), ListItem(-1), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(-1)), ListItem(-2), false, false);
  assert_query_exists(cache.get(AsyncListIndex::After(-2)), ListItem(-1), false, false);
  assert_query_exists(cache.get(AsyncListIndex::After(-1)), ListItem(0), false, false);

  cache.insert(AsyncListIndex::Before(2), ListItem(1), false, false);
  assert_eq!(cache.find(&-2), Some(ListItem(-2)));
  assert_eq!(cache.find(&-1), Some(ListItem(-1)));
  assert_eq!(cache.find(&0), Some(ListItem(0)));
  assert_eq!(cache.find(&1), Some(ListItem(1)));
  assert_eq!(cache.find(&2), Some(ListItem(2)));
  assert_query_exists(cache.get(AsyncListIndex::After(1)), ListItem(2), false, false);
  assert_query_exists(cache.get(AsyncListIndex::After(0)), ListItem(1), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(2)), ListItem(1), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(1)), ListItem(0), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(0)), ListItem(-1), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(-1)), ListItem(-2), false, false);
  assert_query_exists(cache.get(AsyncListIndex::After(-2)), ListItem(-1), false, false);
  assert_query_exists(cache.get(AsyncListIndex::After(-1)), ListItem(0), false, false);

  let mut cache = AsyncListCache::<ListItem>::new();

  cache.insert_detached(ListItem(0));
  assert_eq!(cache.find(&0), Some(ListItem(0)));

  cache.insert(AsyncListIndex::After(0), ListItem(2), false, false);

  assert_eq!(cache.find(&0), Some(ListItem(0)));
  assert_eq!(cache.find(&2), Some(ListItem(2)));
  assert_query_exists(cache.get(AsyncListIndex::After(0)), ListItem(2), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(2)), ListItem(0), false, false);

  cache.insert(AsyncListIndex::Before(0), ListItem(-2), false, false);
  assert_eq!(cache.find(&-2), Some(ListItem(-2)));
  assert_eq!(cache.find(&0), Some(ListItem(0)));
  assert_eq!(cache.find(&2), Some(ListItem(2)));
  assert_query_exists(cache.get(AsyncListIndex::After(0)), ListItem(2), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(2)), ListItem(0), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(0)), ListItem(-2), false, false);
  assert_query_exists(cache.get(AsyncListIndex::After(-2)), ListItem(0), false, false);

  cache.insert(AsyncListIndex::Before(0), ListItem(-1), false, false);
  assert_eq!(cache.find(&-2), Some(ListItem(-2)));
  assert_eq!(cache.find(&-1), Some(ListItem(-1)));
  assert_eq!(cache.find(&0), Some(ListItem(0)));
  assert_eq!(cache.find(&2), Some(ListItem(2)));
  assert_query_exists(cache.get(AsyncListIndex::After(0)), ListItem(2), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(2)), ListItem(0), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(0)), ListItem(-1), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(-1)), ListItem(-2), false, false);
  assert_query_exists(cache.get(AsyncListIndex::After(-2)), ListItem(-1), false, false);
  assert_query_exists(cache.get(AsyncListIndex::After(-1)), ListItem(0), false, false);

  cache.insert(AsyncListIndex::After(0), ListItem(1), false, false);
  assert_eq!(cache.find(&-2), Some(ListItem(-2)));
  assert_eq!(cache.find(&-1), Some(ListItem(-1)));
  assert_eq!(cache.find(&0), Some(ListItem(0)));
  assert_eq!(cache.find(&1), Some(ListItem(1)));
  assert_eq!(cache.find(&2), Some(ListItem(2)));
  assert_query_exists(cache.get(AsyncListIndex::After(1)), ListItem(2), false, false);
  assert_query_exists(cache.get(AsyncListIndex::After(0)), ListItem(1), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(2)), ListItem(1), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(1)), ListItem(0), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(0)), ListItem(-1), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(-1)), ListItem(-2), false, false);
  assert_query_exists(cache.get(AsyncListIndex::After(-2)), ListItem(-1), false, false);
  assert_query_exists(cache.get(AsyncListIndex::After(-1)), ListItem(0), false, false);
}

#[test]
pub fn cache_can_merge() {
  let mut cache = AsyncListCache::<ListItem>::new();

  cache.insert_detached(ListItem(0));
  assert_eq!(cache.find(&0), Some(ListItem(0)));

  cache.insert(AsyncListIndex::After(0), ListItem(1), false, false);

  assert_eq!(cache.find(&0), Some(ListItem(0)));
  assert_eq!(cache.find(&1), Some(ListItem(1)));
  assert_query_exists(cache.get(AsyncListIndex::After(0)), ListItem(1), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(1)), ListItem(0), false, false);

  cache.insert_detached(ListItem(4));
  assert_eq!(cache.find(&4), Some(ListItem(4)));

  cache.insert(AsyncListIndex::Before(4), ListItem(3), false, false);

  assert_eq!(cache.find(&4), Some(ListItem(4)));
  assert_eq!(cache.find(&3), Some(ListItem(3)));
  assert_query_exists(cache.get(AsyncListIndex::After(3)), ListItem(4), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(4)), ListItem(3), false, false);

  cache.insert(AsyncListIndex::Before(3), ListItem(2), false, false);
  cache.insert(AsyncListIndex::After(1), ListItem(2), false, false);

  assert_eq!(cache.find(&4), Some(ListItem(4)));
  assert_eq!(cache.find(&3), Some(ListItem(3)));
  assert_eq!(cache.find(&2), Some(ListItem(2)));
  assert_eq!(cache.find(&1), Some(ListItem(1)));
  assert_eq!(cache.find(&0), Some(ListItem(0)));
  assert_query_exists(cache.get(AsyncListIndex::After(3)), ListItem(4), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(4)), ListItem(3), false, false);
  assert_query_exists(cache.get(AsyncListIndex::After(2)), ListItem(3), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(3)), ListItem(2), false, false);
  assert_query_exists(cache.get(AsyncListIndex::After(1)), ListItem(2), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(2)), ListItem(1), false, false);
  assert_query_exists(cache.get(AsyncListIndex::After(0)), ListItem(1), false, false);
  assert_query_exists(cache.get(AsyncListIndex::Before(1)), ListItem(0), false, false);
}

#[test]
pub fn cache_remove_joins_neighbours() {
  let mut cache = AsyncListCache::<ListItem>::new();

  cache.append_bottom(ListItem(0));
  cache.append_bottom(ListItem(1));
  cache.append_bottom(ListItem(2));

  assert_eq!(cache.remove(&1), Some(ListItem(1)));
  assert_eq!(cache.remove(&1), None);
  assert_eq!(cache.find(&1), None);

  assert_query_exists(cache.get(AsyncListIndex::After(0)), ListItem(2), false, true);
  assert_query_exists(cache.get(AsyncListIndex::Before(2)), ListItem(0), false, false);
  assert_query_exists(cache.get(AsyncListIndex::RelativeToBottom(1)), ListItem(0), false, false);
}

#[test]
pub fn cache_remove_at_bottom_moves_the_bound() {
  let mut cache = AsyncListCache::<ListItem>::new();

  cache.append_bottom(ListItem(0));
  cache.append_bottom(ListItem(1));

  assert_eq!(cache.remove(&1), Some(ListItem(1)));
  assert_eq!(cache.bounded_at_bottom_by(), Some(0));
  assert_query_exists(cache.get(AsyncListIndex::RelativeToBottom(0)), ListItem(0), false, true);
  assert!(matches!(cache.get(AsyncListIndex::After(0)), Exists::No));
}

#[test]
pub fn cache_remove_last_item_forgets_the_bounds() {
  let mut cache = AsyncListCache::<ListItem>::new();

  cache.append_bottom(ListItem(0));
  assert_eq!(cache.remove(&0), Some(ListItem(0)));

  assert_eq!(cache.bounded_at_bottom_by(), None);
  assert_eq!(cache.bounded_at_top_by(), None);
  assert!(matches!(cache.get(AsyncListIndex::RelativeToBottom(0)), Exists::Unknown));

  // The cache is usable again afterwards.
  cache.append_bottom(ListItem(5));
  assert_query_exists(cache.get(AsyncListIndex::RelativeToBottom(0)), ListItem(5), false, true);
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Versioned {
  id: i64,
  version: u32,
}

impl AsyncListItem for Versioned {
  type Identifier = i64;

  fn get_list_identifier(&self) -> Self::Identifier {
    self.id
  }
}

#[test]
pub fn cache_replace_keeps_position_and_ignores_unknown_items() {
  let mut cache = AsyncListCache::<Versioned>::new();

  cache.append_bottom(Versioned { id: 0, version: 1 });
  cache.append_bottom(Versioned { id: 1, version: 1 });

  assert_eq!(cache.replace(Versioned { id: 0, version: 2 }), Some(Versioned { id: 0, version: 1 }));
  assert_eq!(cache.find(&0), Some(Versioned { id: 0, version: 2 }));
  assert_query_exists(cache.get(AsyncListIndex::Before(1)), Versioned { id: 0, version: 2 }, false, false);

  assert_eq!(cache.replace(Versioned { id: 9, version: 1 }), None);
  assert_eq!(cache.find(&9), None);
  assert!(matches!(cache.get(AsyncListIndex::After(1)), Exists::No));
}
