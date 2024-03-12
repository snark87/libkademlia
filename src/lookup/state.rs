use futures::{stream, StreamExt};
use std::{
    collections::{BinaryHeap, HashSet},
    sync::Arc,
};
use tokio::sync::{oneshot, Mutex};

use crate::{
    key::{Distance, Key},
    node::Node,
    params::KeySizeParameters,
    rpc::FindValueResult,
    KademliaParameters,
};

use super::method::LookupMethod;

/// Represents a node along with its distance from a target key in a Kademlia network.
///
/// This struct is used in the context of Kademlia lookup operations to keep track of nodes
/// and their computed distances to a specific target key. The distance metric is crucial
/// for organizing nodes based on their proximity to the target, as Kademlia relies on this
/// ordering to optimize lookup efficiency and network topology.
///
/// # Type Parameters
/// - `P`: Kademlia parameters
/// - `Link`: values required to connect to node (e.g. network address and port)
/// # Fields
/// - `node`: The `Node` struct representing the peer in the Kademlia network. It includes
///   essential information such as the node's unique identifier and network address.
/// - `distance`: A measure of the XOR distance between the node's identifier and the target
///   key. In Kademlia, this distance is used to determine how close a node is to the target,
///   with a smaller distance indicating closer proximity.
///
#[derive(Clone)]
struct NodeWithDistance<P: KeySizeParameters, Link: Clone> {
    node: Arc<Node<P, Link>>,
    distance: Distance<P>,
}

/// Represents the state of a Kademlia lookup operation.
///
/// This struct encapsulates the state necessary to perform iterative node lookups in a Kademlia
/// distributed network. It maintains a list of nodes that are closest to a given target key,
/// updating this list as the lookup process progresses.
///
/// The `LookupState` struct is designed to be used with asynchronous operations, supporting
/// concurrent network queries to efficiently discover the closest nodes. It tracks the nodes
/// that have already been discovered to avoid redundant operations, and it manages a queue of
/// nodes that are pending query, ensuring that the search space is explored efficiently.
///
pub struct LookupState<'k, P: KademliaParameters, M: LookupMethod<P>> {
    key: &'k Key<P>,
    internal_state: Mutex<LookupInternalState<P, M::Link, M::Value>>,
}

struct LookupInternalState<P: KeySizeParameters, Link: Clone, V> {
    done_tx: Option<oneshot::Sender<()>>,
    pending_requests_counter: usize,
    non_queried_nodes: BinaryHeap<NodeWithDistance<P, Link>>,
    queried_nodes_ids: HashSet<Key<P>>,
    discovered_nodes_ids: HashSet<Key<P>>,
    live_nodes: BinaryHeap<NodeWithDistance<P, Link>>,
    found_value: Option<V>,
}

impl<P: KeySizeParameters, Link: Clone, V> LookupInternalState<P, Link, V> {
    fn new(done_tx: oneshot::Sender<()>) -> Self {
        LookupInternalState {
            done_tx: Some(done_tx),
            pending_requests_counter: 0,
            queried_nodes_ids: HashSet::new(),
            non_queried_nodes: BinaryHeap::new(),
            discovered_nodes_ids: HashSet::new(),
            live_nodes: BinaryHeap::new(),
            found_value: None,
        }
    }
}

impl<'k, 'nd, P: KademliaParameters, M: LookupMethod<P>> LookupState<'k, P, M> {
    pub fn new(key: &'k Key<P>, done_tx: oneshot::Sender<()>) -> Self {
        LookupState {
            key: key,
            internal_state: Mutex::new(LookupInternalState::new(done_tx)),
        }
    }

    /// Performs a Kademlia `FIND_NODE` or `FIND_VALUE` remote call for a given node returning `k` closest nodes` to the input key.
    ///
    /// This asynchronous method queries given target node for its `k` closest contacts and updates the state of lookup algorithm.
    /// It does not call nodes which were already called during the same lookup, and it sends done signal if the lookup is finished.
    ///
    /// # Parameters
    /// - `method`: The communication layer used for network operations, which must implement the `LookupMethod` trait,
    /// - `node`: The target node for which the `k`` closest nodes are being sought.
    ///
    /// # Returns
    /// A `Result` containing either:
    /// - `Ok(Vec<Node<P, C::Link>>)`: A vector of the K closest nodes to the target node, as determined by
    ///   the lookup operation. These nodes are returned in order of increasing distance from the target node's ID.
    /// - `Err(C::Error)`: An error of type `C::Error` indicating a failure in the lookup operation. Failures may occur
    ///   due to network issues, timeouts, or if the target node or any intermediate nodes are unresponsive or return
    ///   invalid data.
    ///
    /// # Errors
    /// This method propagates any network or protocol-related errors encountered during the lookup process.
    pub async fn get_k_closest(
        &self,
        method: &M,
        node: Arc<Node<P, M::Link>>,
    ) -> Result<Vec<Arc<Node<P, M::Link>>>, M::Error> {
        let response = self.get_k_closest_internal(method, node).await;
        if self.should_finish().await {
            let mut state = self.internal_state.lock().await;
            state.done_tx.take().map(|tx| tx.send(()).unwrap());
        }
        response
    }

    async fn get_k_closest_internal(
        &self,
        method: &M,
        node: Arc<Node<P, M::Link>>,
    ) -> Result<Vec<Arc<Node<P, M::Link>>>, M::Error> {
        let should_query = self.on_before_request(node.clone()).await;
        if !should_query {
            return Ok(vec![]);
        }
        let response = method.lookup(&node.link).await;
        let response = self.on_response_received(node.clone(), response).await;

        let nodes_or_value = response?;

        match nodes_or_value {
            FindValueResult::ClosestNodes(discovered_nodes) => {
                let new_discovered_nodes = self.filter_new_discovered_nodes(discovered_nodes).await;
                let new_discovered_nodes: Vec<_> = new_discovered_nodes
                    .into_iter()
                    .map(|n| Arc::new(n))
                    .collect();
                stream::iter(new_discovered_nodes.clone())
                    .for_each(|node| self.enqueue_new_contact(node))
                    .await;
                Ok(new_discovered_nodes.clone())
            }
            FindValueResult::FoundValue(value) => {
                self.on_value_found(value).await;
                Ok(vec![])
            }
        }
    }

    pub async fn get_result(&self) -> FindValueResult<P, M::Link, M::Value> {
        let mut state = self.internal_state.lock().await;
        match state.found_value.take() {
            None => {
                let closest_live_nodes = (0..P::K_PARAM)
                    .filter_map(|_| state.live_nodes.pop())
                    .map(|node| node.node.as_ref().clone())
                    .collect();
                FindValueResult::ClosestNodes(closest_live_nodes)
            }
            Some(value) => FindValueResult::FoundValue(value),
        }
    }

    pub async fn get_nodes_to_query(&self) -> Vec<Arc<Node<P, M::Link>>> {
        let mut internal_state = self.internal_state.lock().await;
        let max_nodes_to_pop = Self::get_max_nodes_to_pop(&internal_state);
        let nodes = (0..max_nodes_to_pop)
            .filter_map(|_| internal_state.non_queried_nodes.pop())
            .map(|node| node.node);
        nodes.collect()
    }

    async fn on_value_found(&self, value: M::Value) {
        let mut state = self.internal_state.lock().await;
        state.found_value = Some(value);
        let maybe_done_tx = state.done_tx.take();
        maybe_done_tx.map(|tx| tx.send(()).unwrap());
    }

    fn get_max_nodes_to_pop(state: &LookupInternalState<P, M::Link, M::Value>) -> usize {
        if state.pending_requests_counter > P::ALPHA_PARAM as usize {
            return 0;
        }

        P::ALPHA_PARAM as usize - state.pending_requests_counter
    }

    async fn should_finish(&self) -> bool {
        let state = self.internal_state.lock().await;
        return state.pending_requests_counter == 0 && state.non_queried_nodes.is_empty();
    }

    async fn enqueue_new_contact(&self, node: Arc<Node<P, M::Link>>) {
        let mut state = self.internal_state.lock().await;
        if state.queried_nodes_ids.contains(&node.node_id) {
            return;
        }
        let distance = Distance::between(self.key, &node.node_id);
        state.non_queried_nodes.push(NodeWithDistance {
            node: node,
            distance: distance,
        });
    }

    async fn filter_new_discovered_nodes(
        &self,
        nodes: Vec<Node<P, M::Link>>,
    ) -> Vec<Node<P, M::Link>> {
        let new_discovered_nodes = stream::iter(nodes).filter_map(|node| async {
            if self.on_node_discovered(&node).await {
                return Some(node);
            }

            None
        });

        new_discovered_nodes.collect().await
    }

    async fn on_before_request(&self, node: Arc<Node<P, M::Link>>) -> bool {
        let mut state = self.internal_state.lock().await;
        if state.queried_nodes_ids.contains(&node.node_id) {
            return false;
        }

        state.queried_nodes_ids.insert(node.node_id.clone());
        state.discovered_nodes_ids.insert(node.node_id.clone());
        state.pending_requests_counter += 1;

        true
    }

    async fn on_response_received<T, E>(
        &self,
        node: Arc<Node<P, M::Link>>,
        response: Result<T, E>,
    ) -> Result<T, E> {
        let mut state = self.internal_state.lock().await;
        state.pending_requests_counter -= 1;
        if response.is_ok() {
            let distance = Distance::between(self.key, &node.node_id);
            state.live_nodes.push(NodeWithDistance {
                node: node,
                distance: distance,
            });
        }

        response
    }

    async fn on_node_discovered(&self, node: &Node<P, M::Link>) -> bool {
        let mut internal_state = self.internal_state.lock().await;
        if internal_state.discovered_nodes_ids.contains(&node.node_id) {
            return false;
        }
        internal_state
            .discovered_nodes_ids
            .insert(node.node_id.clone());

        true
    }
}

impl<P: KeySizeParameters, C: Clone> PartialEq for NodeWithDistance<P, C> {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl<P: KeySizeParameters, C: Clone> Eq for NodeWithDistance<P, C> {}

impl<P: KeySizeParameters, C: Clone> PartialOrd for NodeWithDistance<P, C> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<P: KeySizeParameters, C: Clone> Ord for NodeWithDistance<P, C> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.distance.cmp(&self.distance)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::oneshot;
    use tokio_test::{assert_err, assert_ok};

    use crate::lookup::method::FindNodeLookup;
    use crate::lookup::mocks::TestError;

    use super::super::mocks::{MockCommunicator, TestKey, TestLink, TestNode};
    use super::LookupState;

    #[tokio::test]
    async fn when_node_responds_successfully_should_have_no_pending_requests() {
        // Arrange
        let (tx, _rx) = oneshot::channel::<()>();
        let mut mock_communicator = MockCommunicator::new();
        let key = TestKey::new();
        let node = Arc::new(TestNode::new(TestKey::new(), TestLink::Link1));
        let lookup_state = LookupState::new(&key, tx);
        let internal_state = lookup_state.internal_state.lock().await;
        assert_eq!(0, internal_state.pending_requests_counter);
        drop(internal_state);

        mock_communicator
            .expect_get_k_closest()
            .times(1)
            .returning(|_, _| Ok(vec![]));
        let method = FindNodeLookup::new(&key, &mock_communicator);

        // Act
        let _ = lookup_state.get_k_closest(&method, node.clone()).await;

        // Assert
        let internal_state = lookup_state.internal_state.lock().await;
        assert_eq!(0, internal_state.pending_requests_counter);
    }

    #[tokio::test]
    async fn when_communicator_fails_should_have_no_pending_requests() {
        // Arrange
        let (tx, _rx) = oneshot::channel::<()>();
        let mut mock_communicator = MockCommunicator::new();
        let key = TestKey::new();
        let node = Arc::new(TestNode::new(TestKey::new(), TestLink::Link1));
        let lookup_state = LookupState::new(&key, tx);
        let internal_state = lookup_state.internal_state.lock().await;
        assert_eq!(0, internal_state.pending_requests_counter);
        drop(internal_state);

        mock_communicator
            .expect_get_k_closest()
            .times(1)
            .returning(|_, _| Err(TestError::TestError));
        let method = FindNodeLookup::new(&key, &mock_communicator);

        // Act
        let _ = lookup_state.get_k_closest(&method, node.clone()).await;

        // Assert
        let internal_state = lookup_state.internal_state.lock().await;
        assert_eq!(0, internal_state.pending_requests_counter);
    }

    #[tokio::test]
    async fn when_communicator_fails_should_return_err() {
        // Arrange
        let (tx, _rx) = oneshot::channel::<()>();
        let mut mock_communicator = MockCommunicator::new();
        let key = TestKey::new();
        let node = Arc::new(TestNode::new(TestKey::new(), TestLink::Link1));
        let lookup_state = LookupState::new(&key, tx);
        let internal_state = lookup_state.internal_state.lock().await;
        assert_eq!(0, internal_state.pending_requests_counter);
        drop(internal_state);

        mock_communicator
            .expect_get_k_closest()
            .times(1)
            .returning(|_, _| Err(TestError::TestError));
        let method = FindNodeLookup::new(&key, &mock_communicator);

        // Act
        let result = lookup_state.get_k_closest(&method, node.clone()).await;

        // Assert
        assert_err!(result);
    }

    #[tokio::test]
    async fn when_node_responds_with_empty_list_should_return_empty() {
        // Arrange
        let (tx, _rx) = oneshot::channel::<()>();
        let mut mock_communicator = MockCommunicator::new();
        let key = TestKey::new();
        let node = Arc::new(TestNode::new(TestKey::new(), TestLink::Link1));
        let lookup_state = LookupState::new(&key, tx);
        let internal_state = lookup_state.internal_state.lock().await;
        assert_eq!(0, internal_state.pending_requests_counter);
        drop(internal_state);

        mock_communicator
            .expect_get_k_closest()
            .times(1)
            .returning(|_, _| Ok(vec![]));
        let method = FindNodeLookup::new(&key, &mock_communicator);

        // Act
        let result = lookup_state.get_k_closest(&method, node.clone()).await;

        // Assert
        assert_ok!(result.clone());
        let result_nodes: Vec<_> = result.iter().flat_map(|nodes| nodes.iter()).collect();
        assert!(result_nodes.is_empty());
    }

    #[tokio::test]
    async fn when_calling_already_requested_node_should_ignore_call() {
        // Arrange
        let (tx, _rx) = oneshot::channel::<()>();
        let mut mock_communicator = MockCommunicator::new();
        let key = TestKey::new();
        let node = Arc::new(TestNode::new(TestKey::new(), TestLink::Link1));
        let lookup_state = LookupState::new(&key, tx);
        let internal_state = lookup_state.internal_state.lock().await;
        assert_eq!(0, internal_state.pending_requests_counter);
        drop(internal_state);

        mock_communicator
            .expect_get_k_closest()
            .times(1)
            .returning(|_, _| Ok(vec![]));
        let method = FindNodeLookup::new(&key, &mock_communicator);

        // Act
        let _ = lookup_state.get_k_closest(&method, node.clone()).await;
        let _ = lookup_state.get_k_closest(&method, node.clone()).await;

        // Assert
        let internal_state = lookup_state.internal_state.lock().await;
        assert_eq!(0, internal_state.pending_requests_counter);
    }
}
