pub trait ResultExt<T> {
  fn unwrap_with_message(self, message: &str) -> T;
}

impl<T, E> ResultExt<T> for Result<T, E>
where
  E: std::fmt::Debug,
{
  fn unwrap_with_message(self, message: &str) -> T {
    match self {
      Ok(value) => value,
      Err(error) => {
        eprintln!("Error: {}", message);
        eprintln!("{:?}", error);
        panic!();
      }
    }
  }
}
