mod kbucket;

use std::{collections::BinaryHeap, marker::PhantomData};

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::{key::Key, node::Node, rpc::NodeAvailabilityChecker, Communicator, KademliaParameters};

use kbucket::KBucket;

use self::kbucket::{KBucketWithCloseness, ShiftedKey};

#[async_trait]
pub trait RoutingTable<P: KademliaParameters> {
    type Link: Clone;
    type Communicator: Communicator<P, Link = Self::Link>;

    async fn store(&self, communicator: &Self::Communicator, node: Node<P, Self::Link>);
    async fn find_closest_nodes(&self, count: usize, key: &Key<P>) -> Vec<Node<P, Self::Link>>;
    async fn find_by_id(&self, id: &Key<P>) -> Option<Node<P, Self::Link>>;
}

/// represents simplified Kademlia routing table.
/// Nodes are stored in k-buckets according to original Kademlia algorithm specification,
/// but instead of using tree structures, buckets are stored as a plain list.
/// In production-ready version, a memory-efficient tree will be used, but `SimpleRoutingTable`
/// might still be good for smaller networks.
///
/// # Example
/// ```
/// use kademlia::{DefaultKademliaParameters, Key, routing::SimpleRoutingTable, rpc::grpc::GrpcCommunicator};
///
/// type GrpcLink = String;
///
/// let owner_id = Key::new();
/// let routing_table = SimpleRoutingTable::<DefaultKademliaParameters, GrpcLink, GrpcCommunicator<DefaultKademliaParameters>>::new(&owner_id);
/// ```
pub struct SimpleRoutingTable<
    Params: KademliaParameters,
    Link: Clone,
    C: NodeAvailabilityChecker<Link = Link>,
> {
    node_id: Key<Params>,
    buckets: RwLock<Vec<KBucket<Params, Link>>>,
    _communicator: PhantomData<C>,
}

impl<P: KademliaParameters, Link: Clone, C: Communicator<P, Link = Link>>
    SimpleRoutingTable<P, Link, C>
{
    pub fn new(owner_id: &Key<P>) -> Self {
        Self {
            node_id: owner_id.clone(),
            buckets: RwLock::new(vec![KBucket::new()]),
            _communicator: PhantomData,
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

    fn shift_key(&self, key: &Key<P>) -> ShiftedKey<P> {
        ShiftedKey::new(&self.node_id, key)
    }
}

#[async_trait]
impl<P: KademliaParameters, Link: Clone + Send + Sync, C: Communicator<P, Link = Link>>
    RoutingTable<P> for SimpleRoutingTable<P, Link, C>
{
    type Link = Link;
    type Communicator = C;

    async fn store(&self, communicator: &C, node: Node<P, Link>) {
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

    async fn find_closest_nodes(&self, count: usize, key: &Key<P>) -> Vec<Node<P, Link>> {
        let buckets = self.buckets.read().await;
        let ref_buckets: Vec<_> = buckets.iter().map(|b| b).collect();
        let shifted_key = self.shift_key(key);
        Self::find_closest(ref_buckets, &shifted_key)
            .flat_map(|b| &b.nodes)
            .map(|n| n.clone())
            .take(count)
            .collect()
    }

    async fn find_by_id(&self, id: &Key<P>) -> Option<Node<P, Link>> {
        let buckets = self.buckets.read().await;

        let shifted_key = self.shift_key(id);
        let bucket = Self::find_bucket(&buckets, &shifted_key);
        bucket.find_by_id(id).map(|n| n.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use crate::{
        lookup::mocks::{MockCommunicator, TestLink},
        DefaultKademliaParameters, Key, Node,
    };

    use super::*;

    #[tokio::test]
    async fn when_empty_table_find_by_id_should_find_nothing() {
        // Arrange
        let table =
            SimpleRoutingTable::<DefaultKademliaParameters, TestLink, MockCommunicator>::new(
                &Key::new(),
            );

        // Act
        let stored_node = table.find_by_id(&Key::new()).await;
        assert_eq!(None, stored_node);
    }

    #[tokio::test]
    async fn when_node_stored_find_by_id_should_find_it() {
        // Arrange
        let owner_id = Key::new();
        let node_id = Rc::new(Key::new());
        let node_to_store = Node::new(node_id.as_ref().clone(), TestLink::Link1);
        let communicator = MockCommunicator::new();

        let table = SimpleRoutingTable::new(&owner_id);

        // Act
        table.store(&communicator, node_to_store).await;
        let stored_node = table.find_by_id(node_id.as_ref()).await;
        let stored_node_id = stored_node.map(|n| n.node_id);
        assert_eq!(Some(node_id.as_ref().clone()), stored_node_id);
    }

    #[tokio::test]
    async fn when_up_to_k_nodes_stored_find_by_id_should_find_them() {
        // Arrange
        let owner_id = Key::new();
        let table = SimpleRoutingTable::new(&owner_id);

        let mut stored_ids: Vec<Rc<Key<_>>> = Vec::new();
        for _ in 0..20 {
            let node_id = Rc::new(Key::new());
            stored_ids.push(node_id.clone());
            let node_to_store = Node::new(node_id.as_ref().clone(), TestLink::Link1);
            let communicator = MockCommunicator::new();

            // Act
            table.store(&communicator, node_to_store).await;
        }
        for stored_node_id in stored_ids.into_iter() {
            let stored_node = table.find_by_id(stored_node_id.as_ref()).await;
            let node_id = stored_node.map(|n| n.node_id);
            assert_eq!(Some(stored_node_id.as_ref().clone()), node_id);
        }
    }
}
