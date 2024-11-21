use std::{fmt::Debug, future::Future};

use crate::channel::Channel;

pub trait ClientConstructor<C: Client> {
  type ConstructorArguments;
  type ConstructorFailure;

  fn construct(args: Self::ConstructorArguments) -> impl Future<Output = Result<C, Self::ConstructorFailure>>;
}

pub trait Client {
  type Identifier: Sized + Copy + Clone + Debug + Eq + PartialEq;
  type Channel: Channel;

  fn channel(identifier: Self::Identifier) -> impl Future<Output = Option<Self::Channel>>;
}
