#[derive(Clone, Hash, PartialEq, Eq, Copy, Debug)]
pub struct Snowflake {
  pub content: u64,
}

#[allow(clippy::to_string_trait_impl)]
impl ToString for Snowflake {
  fn to_string(&self) -> String {
    self.content.to_string()
  }
}

impl <T> From<T> for Snowflake where T: Into<u64> {
  fn from(content: T) -> Self {
    Snowflake { content: content.into() }
  }
}
