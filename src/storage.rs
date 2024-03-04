use async_trait::async_trait;

use crate::{Key, KeySizeParameters};

#[async_trait]
pub trait KeyValueStorage<P: KeySizeParameters, V> {
    async fn store(key: &Key<P>, value: V);
}