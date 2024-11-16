use std::fmt::{Display, Formatter, Result};

#[derive(Clone, Hash, PartialEq, Eq, Copy, Debug)]
pub struct Snowflake {
  pub content: u64,
}

impl Display for Snowflake {
  fn fmt(&self, f: &mut Formatter<'_>) -> Result {
    write!(f, "{}", self.content)
  }
}
