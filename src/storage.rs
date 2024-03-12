use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::{Key, KeySizeParameters};

#[async_trait]
pub trait KeyValueStorage<P: KeySizeParameters> {
    type Value;

    async fn store(&self, key: &Key<P>, value: Self::Value);
}

pub struct HashMapKVS<P: KeySizeParameters, V: Send> {
    values: Mutex<HashMap<Key<P>, V>>,
}

impl<P: KeySizeParameters, V: Send> HashMapKVS<P, V> {
    pub fn new() -> Self {
        Self {
            values: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl<P: KeySizeParameters, V: Send> KeyValueStorage<P> for HashMapKVS<P, V> {
    type Value = V;

    async fn store(&self, key: &Key<P>, value: Self::Value) {
        let mut values = self.values.lock().await;
        values.insert(key.clone(), value);
    }
}
