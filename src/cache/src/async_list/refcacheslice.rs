use scope_chat::async_list::AsyncListIndex;

pub struct CacheReferencesSlice<I: Clone + Eq + PartialEq> {
    pub is_bounded_at_top: bool,
    pub is_bounded_at_bottom: bool,
  
    // the vec's 0th item is the top, and it's last item is the bottom
    // the vec MUST NOT be empty.
    pub (super) item_references: Vec<I>,
}

impl<I: Clone + Eq + PartialEq> CacheReferencesSlice<I> {    
    fn find_index_of(&self, item: I) -> Option<usize> {
        for (haystack, index) in self.item_references.iter().zip(0..) {
            if (*haystack == item) {
                return Some(index)
            }
        }

        None
    }

    fn get_index(&self, index: AsyncListIndex<I>) -> Option<isize> {
        match index {
            AsyncListIndex::RelativeToBottom(count) if self.is_bounded_at_bottom => {
                Some((self.item_references.len() as isize) - (count as isize))
            }

            AsyncListIndex::RelativeToTop(count) if self.is_bounded_at_top => {
                Some(count as isize)
            }

            AsyncListIndex::After(item) => {
                Some((self.find_index_of(item)? as isize) + 1)
            }

            AsyncListIndex::Before(item) => {
                Some((self.find_index_of(item)? as isize) - 1)
            }

            _ => None,
        }
    }

    pub fn get(&self, index: AsyncListIndex<I>) -> Option<I> {
        let index = self.get_index(index)?;

        if index < 0 {
            return None;
        }

        self.item_references.get(index as usize).cloned()
    }

    pub fn can_insert(&self, index: AsyncListIndex<I>) -> Option<Position> {
        match index {
            AsyncListIndex::After(item) => self.find_index_of(item).map(|idx| if idx == (self.item_references.len() - 1) { Position::After } else { Position::Inside }),
            AsyncListIndex::Before(item) => self.find_index_of(item).map(|idx| if idx == 0 { Position::Before } else { Position::Inside }),

            _ => panic!("TODO: Figure out what well-defined behaviour should occur for inserting relative to top or bottom")
        }
    }

    pub fn insert(&mut self, index: AsyncListIndex<I>, value: I) {
        match index {
            AsyncListIndex::After(item) => {
                let i = self.find_index_of(item).unwrap();

                self.item_references.insert(i + 1, value);
            },
            AsyncListIndex::Before(item) => {
                let i = self.find_index_of(item).unwrap();

                self.item_references.insert(i, value);
            },

            _ => panic!("TODO: Figure out what well-defined behaviour should occur for inserting relative to top or bottom")
        }
    }
}

#[derive(Clone, Copy)]
pub enum Position {
    Before,
    Inside,
    After,
}