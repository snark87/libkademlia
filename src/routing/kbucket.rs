use std::collections::VecDeque;

use bitvec::prelude::*;
use generic_array::GenericArray;

use crate::{
    key::{Distance, KeyLike},
    node::Node,
    Communicator, KademliaParameters, Key, KeySizeParameters,
};

/// Represents bucket of maximum k elements in Kademlia algorithm to store nodes
///
/// ```compile_fail
/// # // should not be accessible outside of crate
/// # use kademlia::{DefaultKademliaParameters, routing::kbucket::KBucket};
/// # let bucket = KBucket::<DefaultKademliaParameters, String>::new();
/// ```
/// ```compile_fail
/// # // should not be accessible outside of crate
/// # use kademlia::{DefaultKademliaParameters, routing::KBucket};
/// # let bucket = KBucket::<DefaultKademliaParameters, String>::new();
/// ````
pub struct KBucket<Params: KademliaParameters, Link: Clone> {
    pub range: KBucketRange,
    pub nodes: VecDeque<Node<Params, Link>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ShiftedKey<Params: KeySizeParameters>(GenericArray<u8, Params::KeySize>);

impl<P: KeySizeParameters> AsRef<[u8]> for ShiftedKey<P> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl<P: KeySizeParameters> KeyLike<P> for ShiftedKey<P> {}

impl<P: KeySizeParameters> ShiftedKey<P> {
    fn zero() -> Self {
        ShiftedKey(GenericArray::default())
    }
}

impl<P: KeySizeParameters> From<&Key<P>> for ShiftedKey<P> {
    fn from(value: &Key<P>) -> Self {
        ShiftedKey::new(value, &Key::zero())
    }
}

impl<P: KeySizeParameters> ShiftedKey<P> {
    pub fn new(key1: &Key<P>, key2: &Key<P>) -> Self {
        let distance = Distance::between(key1, key2);
        Self(distance.0)
    }
}

pub struct KBucketRange {
    prefix: BitVec<u8>,
}

impl<P: KeySizeParameters> From<&ShiftedKey<P>> for BitVec<u8> {
    fn from(value: &ShiftedKey<P>) -> Self {
        BitVec::from(value.as_bits())
    }
}

impl<P: KeySizeParameters> From<&BitVec<u8>> for ShiftedKey<P> {
    fn from(value: &BitVec<u8>) -> Self {
        let slice: &[u8] = value.as_raw_slice();
        let mut data = GenericArray::default();
        for (i, b) in data.iter_mut().enumerate() {
            *b = slice.get(i).map(|x| *x).unwrap_or(0);
        }
        ShiftedKey(data)
    }
}

impl KBucketRange {
    pub fn full() -> Self {
        Self {
            prefix: BitVec::new(),
        }
    }

    pub fn contains<P: KeySizeParameters>(&self, shifted_key: &ShiftedKey<P>) -> bool {
        BitVec::from(shifted_key).starts_with(&self.prefix)
    }

    fn closeness<P: KeySizeParameters>(&self, shifted_key: &ShiftedKey<P>) -> Distance<P> {
        Distance::between(shifted_key, &ShiftedKey::from(&self.prefix))
    }

    fn split(&mut self) -> Self {
        let mut new_prefix = self.prefix.clone();
        self.prefix.push(false);
        new_prefix.push(true);

        Self { prefix: new_prefix }
    }
}
pub struct KBucketWithCloseness<'a, P: KademliaParameters, Link: Clone> {
    pub bucket: &'a KBucket<P, Link>,
    closeness: Distance<P>,
}

impl<P: KademliaParameters, Link: Clone> KBucket<P, Link> {
    pub fn new() -> Self {
        Self {
            range: KBucketRange::full(),
            nodes: VecDeque::new(),
        }
    }

    pub fn update(&mut self, node: &Node<P, Link>) -> bool {
        let index = self
            .nodes
            .iter()
            .enumerate()
            .find(|&(_, el)| &el.node_id == &node.node_id)
            .map(|(i, _)| i);
        let maybe_node = index.and_then(|i| self.nodes.remove(i));
        if let Some(mut el) = maybe_node {
            el.link = node.link.clone();

            self.nodes.push_front(el);
            return true;
        }

        false
    }

    pub fn find_by_id<'a>(&'a self, node_id: &Key<P>) -> Option<&'a Node<P, Link>> {
        self.nodes
            .iter()
            .find(|el| &el.node_id == node_id)
            .map(|el| el)
    }

    /// Stores node in the bucket if there is enough capacity
    ///
    /// # Return value
    /// - `Some<KBucket>` containing splitted bucket
    /// - `None` if bucket is not splitted
    pub async fn store<C: Communicator<P, Link = Link>>(
        &mut self,
        communicator: &C,
        node: Node<P, Link>,
    ) -> Option<Self> {
        if self.nodes.len() < P::K_PARAM as usize {
            self.do_store(node);
            return None;
        }

        let least_recently_seen = self
            .least_recently_seen()
            .expect("nodes should contain K elements");
        let ping_result = communicator.ping(&least_recently_seen.link).await;
        match ping_result {
            Err(_) => {
                self.evict_least_recently_seen();
                self.do_store(node);
                return None;
            }
            Ok(_) => {
                return self.maybe_split_bucket().map(|mut new_bucket| {
                    new_bucket.do_store(node);
                    new_bucket
                });
            }
        }
    }

    fn maybe_split_bucket(&mut self) -> Option<Self> {
        if !self.contains_owner() {
            return None;
        }
        let split_range = self.range.split();

        Some(Self {
            range: split_range,
            nodes: VecDeque::new(),
        })
    }

    fn do_store(&mut self, node: Node<P, Link>) {
        self.nodes.push_front(node);
    }

    fn contains_owner(&self) -> bool {
        let shifted_key = ShiftedKey::<P>::zero();
        self.range.contains(&shifted_key)
    }

    pub fn least_recently_seen(&self) -> Option<&Node<P, Link>> {
        self.nodes.back()
    }

    pub fn evict_least_recently_seen(&mut self) {
        self.nodes.pop_back();
    }
}

impl<'a, P: KademliaParameters, Link: Clone> Ord for KBucketWithCloseness<'a, P, Link> {
    /// Bucket `B1` is consider greater than bucket `B2` if it is closer to the key
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.closeness.cmp(&self.closeness)
    }
}

impl<'a, P: KademliaParameters, Link: Clone> PartialOrd for KBucketWithCloseness<'a, P, Link> {
    /// Bucket `B1` is consider greater than bucket `B2` if it is closer to the key
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a, P: KademliaParameters, Link: Clone> PartialEq for KBucketWithCloseness<'a, P, Link> {
    /// Two buckets are considered equal if they are equally close to the key
    fn eq(&self, other: &Self) -> bool {
        self.closeness == other.closeness
    }
}
impl<'a, P: KademliaParameters, Link: Clone> Eq for KBucketWithCloseness<'a, P, Link> {}

impl<'a, P: KademliaParameters, L: Clone> KBucketWithCloseness<'a, P, L> {
    pub fn new(bucket: &'a KBucket<P, L>, shifted_key: &ShiftedKey<P>) -> Self {
        Self {
            bucket: bucket,
            closeness: bucket.range.closeness(&shifted_key),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{lookup, DefaultKademliaParameters, KademliaParameters, Key, Node};
    use bitvec::prelude::*;

    use super::{KBucket, KBucketRange, ShiftedKey};

    #[test]
    fn from_key_to_bitvec_and_back_should_not_change() {
        let key = &Key::<DefaultKademliaParameters>::new();
        let shifted_key: ShiftedKey<_> = key.into();
        let bitvec: BitVec<u8> = (&shifted_key).into();
        let shifted_key_again: ShiftedKey<_> = (&bitvec).into();
        assert_eq!(shifted_key, shifted_key_again);
    }

    #[test]
    fn full_range_should_contain_all_keys() {
        let zero = Key::zero();
        let shifted_zero = ShiftedKey::new(&zero, &zero);

        let full_range = KBucketRange::full();
        assert!(full_range.contains(&shifted_zero));

        for _ in 0..1024 {
            let key = Key::<DefaultKademliaParameters>::new();
            let shifted_key = ShiftedKey::new(&key, &zero);
            assert!(full_range.contains(&shifted_key));
        }
    }

    #[tokio::test]
    async fn kbucket_when_node_is_stored_can_find_it_by_id() {
        // Arrange
        let communicator = lookup::mocks::MockCommunicator::new();

        let node_id = Key::<DefaultKademliaParameters>::new();
        let link = lookup::mocks::TestLink::Link1;
        let node = Node::new(node_id.clone(), link.clone());
        let mut kbucket = KBucket::new();

        // Act
        kbucket.store(&communicator, node).await;

        // Assert
        let node = kbucket.find_by_id(&node_id).unwrap();
        assert_eq!(link, node.link);
    }

    #[tokio::test]
    async fn kbucket_when_least_recently_seen_alive_cannot_contain_more_than_k_nodes() {
        // Arrange
        let mut communicator = lookup::mocks::MockCommunicator::new();
        communicator.expect_ping().return_const(Ok(()));

        let k = DefaultKademliaParameters::K_PARAM;
        let mut kbucket = KBucket::new();
        let link = lookup::mocks::TestLink::Link1;
        for _ in 0..k {
            let node_id = Key::<DefaultKademliaParameters>::new();
            let node = Node::new(node_id.clone(), link.clone());
            let _ = kbucket.store(&communicator, node).await;
            assert!(kbucket.nodes.len() <= k as usize);
        }

        let node_id = Key::<DefaultKademliaParameters>::new();
        let node = Node::new(node_id.clone(), link.clone());
        let _ = kbucket.store(&communicator, node).await;
        assert!(kbucket.nodes.len() <= k as usize);
    }

    #[tokio::test]
    async fn kbucket_when_least_recently_seen_dead_cannot_contain_more_than_k_nodes() {
        // Arrange
        let mut communicator = lookup::mocks::MockCommunicator::new();
        communicator
            .expect_ping()
            .return_const(Err(lookup::mocks::TestError::TestError));

        let k = DefaultKademliaParameters::K_PARAM;
        let mut kbucket = KBucket::new();
        let link = lookup::mocks::TestLink::Link1;
        for _ in 0..k {
            let node_id = Key::<DefaultKademliaParameters>::new();
            let node = Node::new(node_id.clone(), link.clone());
            let maybe_bucket = kbucket.store(&communicator, node).await;
            assert!(kbucket.nodes.len() <= k as usize);
            assert!(maybe_bucket.is_none());
        }

        let node_id = Key::<DefaultKademliaParameters>::new();
        let node = Node::new(node_id.clone(), link.clone());

        // Act
        let _ = kbucket.store(&communicator, node).await;

        // Assert
        assert!(kbucket.nodes.len() <= k as usize);
    }
}
