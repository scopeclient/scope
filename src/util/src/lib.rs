pub trait ResultExt<T> {}

impl<T, E> ResultExt<T> for Result<T, E> where E: std::fmt::Debug {}
