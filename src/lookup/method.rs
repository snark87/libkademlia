use async_trait::async_trait;

use crate::{rpc::FindValueResult, Communicator, Key, KeySizeParameters};

/// Generalizes FIND_NODE and FIND_VALUE lookups
#[async_trait]
pub trait LookupMethod<P: KeySizeParameters> {
    type Link: Clone;
    type Communicator: Communicator<P, Link = Self::Link>;
    type Error;
    type Value;

    async fn lookup(
        &self,
        link: &Self::Link,
    ) -> Result<FindValueResult<P, Self::Link, Self::Value>, Self::Error>;

    fn communicator(&self) -> &Self::Communicator;
}

/// represents lookup method for `FIND_NODE` procedure
pub struct FindNodeLookup<'a, P: KeySizeParameters, C: Communicator<P>> {
    communicator: &'a C,
    key: &'a Key<P>,
}

impl<'a, P: KeySizeParameters, C: Communicator<P>> FindNodeLookup<'a, P, C> {
    pub fn new(key: &'a Key<P>, communicator: &'a C) -> Self {
        Self {
            key: key,
            communicator: communicator,
        }
    }
}

/// represents lookup method for `FIND_VALUE` procedure
pub struct FindValueLookup<'a, P: KeySizeParameters, C: Communicator<P>> {
    communicator: &'a C,
    key: &'a Key<P>,
}

impl<'a, P: KeySizeParameters, C: Communicator<P>> FindValueLookup<'a, P, C> {
    pub fn new(key: &'a Key<P>, communicator: &'a C) -> Self {
        Self {
            key: key,
            communicator: communicator,
        }
    }
}

#[async_trait]
impl<'a, P: KeySizeParameters, C: Communicator<P>> LookupMethod<P> for FindNodeLookup<'a, P, C> {
    type Link = C::Link;
    type Value = C::Value;
    type Error = C::Error;
    type Communicator = C;

    async fn lookup(
        &self,
        link: &Self::Link,
    ) -> Result<FindValueResult<P, C::Link, C::Value>, C::Error> {
        let response = self.communicator.get_k_closest(&link, self.key).await?;
        Ok(FindValueResult::ClosestNodes(response))
    }

    fn communicator(&self) -> &C {
        self.communicator
    }
}

#[async_trait]
impl<'a, P: KeySizeParameters, C: Communicator<P>> LookupMethod<P> for FindValueLookup<'a, P, C> {
    type Link = C::Link;
    type Value = C::Value;
    type Error = C::Error;
    type Communicator = C;

    async fn lookup(
        &self,
        link: &Self::Link,
    ) -> Result<FindValueResult<P, C::Link, C::Value>, C::Error> {
        self.communicator.find_value(&link, self.key).await
    }

    fn communicator(&self) -> &C {
        self.communicator
    }
}
