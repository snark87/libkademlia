mod kbucket;

use std::collections::BinaryHeap;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::{
    key::Key, node::Node, Communicator, KademliaParameters
};

use kbucket::KBucket;

use self::kbucket::{KBucketWithCloseness, ShiftedKey};

#[async_trait]
pub trait RoutingTable {
    type Key: Clone;
    type Node: Clone;

    async fn store(&self, node: Self::Node);
    fn find_closest_nodes(&self, count: usize, key: &Self::Key) -> Vec<Self::Node>;
    fn find_by_id(&self, id: &Self::Key) -> Option<Self::Node>;
}

/// represents simplified Kademlia routing table.
/// Nodes are stored in k-buckets according to original Kademlia algorithm specification,
/// but instead of using tree structures, buckets are stored as a plain list.
/// In production-ready version, a memory-efficient tree will be used, but `SimpleRoutingTable`
/// might still be good for smaller networks.
///
/// # Example
/// ```
/// use kademlia::{DefaultKademliaParameters, Key, routing::SimpleRoutingTable};
///
/// // represents link (e.g. ip address and tcp port)
/// #[derive(Clone)]
/// struct MyLink {
/// // definition of `MyLink`
/// }
///
/// let owner_id = Key::new();
/// let routing_table = SimpleRoutingTable::<DefaultKademliaParameters, MyLink>::new(&owner_id);
/// ```
pub struct SimpleRoutingTable<Params: KademliaParameters, Link: Clone> {
    node_id: Key<Params>,
    buckets: RwLock<Vec<KBucket<Params, Link>>>,
}

impl<P: KademliaParameters, Link: Clone> SimpleRoutingTable<P, Link> {
    pub fn new(owner_id: &Key<P>) -> Self {
        SimpleRoutingTable {
            node_id: owner_id.clone(),
            buckets: RwLock::new(vec![KBucket::new()]),
        }
    }

    fn find_closest<'a>(
        buckets: Vec<&'a KBucket<P, Link>>,
        shifted_key: &ShiftedKey<P>,
    ) -> impl Iterator<Item = &'a KBucket<P, Link>> + 'a {
        let mut heap = BinaryHeap::new();
        for bucket in buckets {
            let item = KBucketWithCloseness::new(bucket, shifted_key);
            heap.push(item);
        }

        heap.into_iter().map(|b| b.bucket)
    }

    fn find_bucket<'a>(
        buckets: &'a Vec<KBucket<P, Link>>,
        shifted_key: &ShiftedKey<P>,
    ) -> &'a KBucket<P, Link> {
        buckets
            .iter()
            .find(|&b| b.range.contains(shifted_key))
            .unwrap() // there should be always a bucket to contain a key
    }

    fn find_bucket_mut<'a>(
        buckets: &'a mut Vec<KBucket<P, Link>>,
        shifted_key: &ShiftedKey<P>,
    ) -> &'a mut KBucket<P, Link> {
        buckets
            .iter_mut()
            .find(|b| b.range.contains(shifted_key))
            .unwrap() // there should be always a bucket to contain a key
    }

    pub async fn store<C: Communicator<P, Link=Link>>(&self, communicator: &C, node: Node<P, Link>) {
        let mut buckets = self.buckets.write().await;
        let shifted_key = self.shift_key(&node.node_id);
        let bucket = Self::find_bucket_mut(&mut buckets, &shifted_key);
        if bucket.update(&node) {
            return;
        }

        let maybe_new_bucket = bucket.store(communicator, node).await;
        if let Some(new_bucket) = maybe_new_bucket {
            buckets.push(new_bucket);
        }
    }

    pub fn find_closest_nodes(&self, count: usize, key: &Key<P>) -> Vec<Node<P, Link>> {
        let buckets = self.buckets.blocking_read();
        let ref_buckets: Vec<_> = buckets.iter().map(|b| b).collect();
        let shifted_key = self.shift_key(key);
        Self::find_closest(ref_buckets, &shifted_key)
            .flat_map(|b| &b.nodes)
            .map(|n| n.clone())
            .take(count)
            .collect()
    }

    pub fn find_by_id(&self, id: &Key<P>) -> Option<Node<P, Link>> {
        let buckets = self.buckets.blocking_read();

        let shifted_key = self.shift_key(id);
        let bucket = Self::find_bucket(&buckets, &shifted_key);
        bucket.find_by_id(id).map(|n| n.clone())
    }

    fn shift_key(&self, key: &Key<P>) -> ShiftedKey<P> {
        ShiftedKey::new(&self.node_id, key)
    }
}

