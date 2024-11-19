use std::fmt::Debug;

use scope_chat::async_list::{AsyncListIndex, AsyncListItem, AsyncListResult};

use crate::async_list::{refcacheslice::Exists, AsyncListCache};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct ListItem(i64);

impl AsyncListItem for ListItem {
  type Identifier = i64;

  fn get_list_identifier(&self) -> Self::Identifier {
    self.0
  }
}

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
