use std::sync::Arc;

use futures::{stream, Future, FutureExt, Stream, StreamExt};
use tokio::sync::{mpsc::Sender, oneshot};

use crate::{
    key::Key,
    node::Node,
    routing::RoutingTable,
    rpc::{Communicator, FindValueResult},
    KademliaParameters,
};

use self::{
    method::{FindNodeLookup, FindValueLookup, LookupMethod},
    state::LookupState,
    streams::create_channel_stream,
};

mod method;
mod state;
mod streams;

#[cfg(test)]
pub mod mocks;

/// Performs a Kademlia node lookup procedure.
///
/// This function implements the asynchronous iterative node lookup algorithm as described in the Kademlia
/// protocol. It is used to find the `k` closest nodes to a given target node ID in the distributed
/// network. The algorithm progressively queries nodes that are closer to the target, updating its
/// list of closest nodes based on responses received until it converges on the `k` closest nodes.
///
/// # Type Parameters
/// - `P`: Kademlia parameters
/// - `C`: Type of communicator implementing network requests between distributed nodes
///
/// # Parameters
/// - `communicator`: communicator used to reach remote nodes.
/// - `routing_table`: routing table storing discovered nodes
/// - `key`: Target key for which closest nodes are being searched.
///
/// # Returns
/// A `Vec<Node>` containing the `k` closest nodes to the target `key`, sorted by their distance from
/// the target.
///
pub async fn kademlia_node_lookup<P: KademliaParameters, C: Communicator<P>>(
    communicator: &C,
    routing_table: &impl RoutingTable<Node = Node<P, C::Link>, Key = Key<P>>,
    key: &Key<P>,
) -> Vec<Node<P, C::Link>> {
    let method = FindNodeLookup::new(key, communicator);
    if let FindValueResult::ClosestNodes(nodes) =
        kademlia_node_lookup_internal(&method, routing_table, key).await
    {
        nodes
    } else {
        panic!("FindNode returned value: this should never happen")
    }
}

pub async fn kademlia_find_value<P: KademliaParameters, C: Communicator<P>>(
    communicator: &C,
    routing_table: &impl RoutingTable<Node = Node<P, C::Link>, Key = Key<P>>,
    key: &Key<P>,
) -> Option<C::Value> {
    let method = FindValueLookup::new(key, communicator);
    match kademlia_node_lookup_internal(&method, routing_table, key).await {
        FindValueResult::FoundValue(value) => Some(value),
        _ => None,
    }
}

async fn kademlia_node_lookup_internal<P: KademliaParameters, M: LookupMethod<P>>(
    method: &M,
    routing_table: &impl RoutingTable<Node = Node<P, M::Link>, Key = Key<P>>,
    key: &Key<P>,
) -> FindValueResult<P, M::Link, M::Value> {
    // Kademlia lookup is a recursive algorithm, we need a way to stop the recursion
    let (done_tx, done_rx) = oneshot::channel();

    let state = LookupState::new(key, done_tx);
    let closest = routing_table
        .find_closest_nodes(P::ALPHA_PARAM as usize, key)
        .await;
    if closest.is_empty() {
        return FindValueResult::ClosestNodes(vec![]);
    }

    let (nodes_to_request_stream, to_request_tx) = create_channel_stream(P::ALPHA_PARAM as usize);

    let tx = to_request_tx.clone();
    let query_initial_contacts_fut = stream::iter(closest).for_each(|node| {
        let tx = tx.clone();
        async move {
            let _ = tx.send(Arc::new(node)).await;
        }
    });

    let discovered_contacts_stream = discover_new_contacts(method, &state, nodes_to_request_stream);
    let discovered_contacts_stream = store_contacts(routing_table, discovered_contacts_stream);

    // recursion step
    let query_discovered_contacts_fut = query_discovered_contacts(
        &state,
        to_request_tx.clone(),
        discovered_contacts_stream.take_until(done_rx),
    );

    query_initial_contacts_fut.await;
    query_discovered_contacts_fut.await;
    state.get_result().await
}

fn store_contacts<'a, P: KademliaParameters + 'a, Link: Clone + 'a>(
    routing_table: &'a impl RoutingTable<Node = Node<P, Link>, Key = Key<P>>,
    contacts: impl Stream<Item = Arc<Node<P, Link>>> + 'a,
) -> impl Stream<Item = Arc<Node<P, Link>>> + 'a {
    contacts.then(move |node| async {
        routing_table.store(node.as_ref().clone()).await;
        node
    })
}

fn query_discovered_contacts<'a, 'b, P: KademliaParameters, M: LookupMethod<P>>(
    state: &'a LookupState<'b, P, M>,
    to_request_tx: Sender<Arc<Node<P, M::Link>>>,
    contacts: impl Stream<Item = Arc<Node<P, M::Link>>> + 'a,
) -> impl Future<Output = ()> + 'a {
    contacts.for_each(move |_| {
        let tx = to_request_tx.clone();
        async move {
            let contacts_to_query = state.get_nodes_to_query().await;
            for contact in contacts_to_query {
                let _ = tx.send(contact).await;
            }
        }
    })
}

/// Sends request for `k`` closest elements to nodes from an input stream `nodes_to_request` using given `Communicator`.
/// Closest elements are piped to the first of output streams
/// If an input node responds without error its reference is sent to the second output stream
///
/// # Arguments
/// * `communicator` - Communicator used to send requests
/// * `key` - `key` used to find `k`` closest nodes
/// * `nodes_to_request` - stream of nodes to send requests to.
///
/// # Returns
/// * Stream consisting of discovered closest nodes from responses.
fn discover_new_contacts<'a, 'b, P: KademliaParameters, M: LookupMethod<P>>(
    method: &'a M,
    state: &'a LookupState<'a, P, M>,
    nodes_to_request: impl Stream<Item = Arc<Node<P, M::Link>>> + 'a,
) -> impl Stream<Item = Arc<Node<P, M::Link>>> + 'a {
    nodes_to_request.flat_map(move |node| {
        let closest_nodes_stream = state.get_k_closest(method, node).into_stream();

        closest_nodes_stream.flat_map(|resp| match resp {
            Ok(nodes) => stream::iter(nodes),
            Err(_) => stream::iter(vec![]),
        })
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::key::Distance;

    use super::mocks::*;
    use super::*;

    #[tokio::test]
    async fn lookup_empty() {
        let key = TestKey::new();
        let communicator = MockCommunicator::new();
        let mut routing_table = MockRoutingTable::new();
        routing_table
            .expect_find_closest_nodes()
            .once()
            .return_const(vec![]);
        let result = kademlia_node_lookup(&communicator, &routing_table, &key).await;
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn lookup_one_closest_with_empty_response() {
        let key = TestKey::new();
        let key_candidate = TestKey::new();
        let mut communicator = MockCommunicator::new();
        let mut routing_table = MockRoutingTable::new();
        routing_table
            .expect_find_closest_nodes()
            .once()
            .return_const(vec![Node::new(key_candidate.clone(), TestLink::Link1)]);
        communicator
            .expect_get_k_closest()
            .once()
            .return_const(Ok(vec![]));
        let result = kademlia_node_lookup(&communicator, &routing_table, &key).await;
        let actual_ids: Vec<_> = result.iter().map(|n| n.node_id.clone()).collect();
        assert_eq!(vec![key_candidate.clone()], actual_ids);
    }

    #[tokio::test]
    async fn lookup_one_closest_with_error_response() {
        let key = TestKey::new();
        let key_candidate = TestKey::new();
        let mut communicator = MockCommunicator::new();
        let mut routing_table = MockRoutingTable::new();
        routing_table
            .expect_find_closest_nodes()
            .once()
            .return_const(vec![Node::new(key_candidate.clone(), TestLink::Link1)]);
        communicator
            .expect_get_k_closest()
            .once()
            .return_const(Err(TestError::TestError));
        let result = kademlia_node_lookup(&communicator, &routing_table, &key).await;
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn lookup_one_closest_and_chain_requests() {
        let key = TestKey::new();
        let key_candidate_1 = TestKey::new();
        let key_candidate_2 = TestKey::new();
        let mut communicator = MockCommunicator::new();
        let mut routing_table = MockRoutingTable::new();
        routing_table.expect_store().times(1).return_const(());
        routing_table
            .expect_find_closest_nodes()
            .once()
            .return_const(vec![Node::new(key_candidate_1.clone(), TestLink::Link1)]);
        let k2 = key_candidate_2.clone();
        communicator
            .expect_get_k_closest()
            .times(2)
            .returning(move |contact, _| match contact {
                TestLink::Link1 => Ok(vec![Node::new(k2.clone(), TestLink::Link2)]),
                TestLink::Link2 => Ok(vec![]),
                _ => Err(TestError::TestError),
            });
        let result = kademlia_node_lookup(&communicator, &routing_table, &key).await;
        let expected_ids: HashSet<_> = vec![key_candidate_1, key_candidate_2].into_iter().collect();
        let actual_ids: HashSet<_> = result.into_iter().map(|n| n.node_id.clone()).collect();
        assert_eq!(expected_ids, actual_ids);
    }

    #[tokio::test]
    async fn lookup_one_closest_and_duplicates_should_be_ordered() {
        let key = TestKey::new();
        let key_candidates: Vec<TestKey> = (0..4).map(|_| TestKey::new()).collect();
        let key_clones = key_candidates.clone();
        let mut communicator = MockCommunicator::new();
        let mut routing_table = MockRoutingTable::new();
        routing_table.expect_store().times(3).return_const(());
        routing_table
            .expect_find_closest_nodes()
            .once()
            .return_const(vec![Node::new(key_candidates[0].clone(), TestLink::Link1)]);
        communicator
            .expect_get_k_closest()
            .times(4)
            .returning(move |contact, _| match contact {
                TestLink::Link1 => Ok(vec![
                    Node::new(key_clones[1].clone(), TestLink::Link2),
                    Node::new(key_clones[2].clone(), TestLink::Link3),
                ]),
                TestLink::Link2 => Ok(vec![
                    Node::new(key_clones[1].clone(), TestLink::Link2),
                    Node::new(key_clones[3].clone(), TestLink::Link4),
                ]),
                TestLink::Link3 => Ok(vec![
                    Node::new(key_clones[0].clone(), TestLink::Link1),
                    Node::new(key_clones[3].clone(), TestLink::Link4),
                ]),
                TestLink::Link4 => Ok(vec![Node::new(key_clones[2].clone(), TestLink::Link3)]),
            });
        let result = kademlia_node_lookup(&communicator, &routing_table, &key).await;
        let expected_ids: HashSet<_> = key_candidates.into_iter().collect();
        let actual_ids: HashSet<_> = result
            .clone()
            .into_iter()
            .map(|n| n.node_id.clone())
            .collect();
        let distances: Vec<_> = result
            .iter()
            .map(|n| Distance::between(&key, &n.node_id))
            .collect();
        assert_eq!(expected_ids, actual_ids);
        assert!(distances.windows(2).all(|w| w[0] < w[1]));
    }

    #[tokio::test]
    async fn find_value_empty() {
        let key = TestKey::new();
        let communicator = MockCommunicator::new();
        let mut routing_table = MockRoutingTable::new();
        routing_table
            .expect_find_closest_nodes()
            .once()
            .return_const(vec![]);
        let result = kademlia_find_value(&communicator, &routing_table, &key).await;
        assert_eq!(None, result);
    }

    #[tokio::test]
    async fn find_value_when_one_closest_with_value_it_should_return_value() {
        // Arrange
        let value = String::from("test value");
        let key = TestKey::new();
        let key_candidate = TestKey::new();
        let mut communicator = MockCommunicator::new();
        let mut routing_table = MockRoutingTable::new();
        routing_table
            .expect_find_closest_nodes()
            .once()
            .return_const(vec![Node::new(key_candidate.clone(), TestLink::Link1)]);

        let return_value = value.clone();
        communicator
            .expect_find_value()
            .once()
            .returning(move |_, _| Ok(FindValueResult::FoundValue(return_value.clone())));

        // Act
        let result = kademlia_find_value(&communicator, &routing_table, &key).await;

        // Assert
        assert_eq!(Some(value), result);
    }

    #[tokio::test]
    async fn find_value_when_one_closest_with_error_response_it_should_return_none() {
        // Arrange
        let key = TestKey::new();
        let key_candidate = TestKey::new();
        let mut communicator = MockCommunicator::new();
        let mut routing_table = MockRoutingTable::new();
        routing_table
            .expect_find_closest_nodes()
            .once()
            .return_const(vec![Node::new(key_candidate.clone(), TestLink::Link1)]);
        communicator
            .expect_find_value()
            .once()
            .returning(|_, _| Err(TestError::TestError));

        // Act
        let result = kademlia_find_value(&communicator, &routing_table, &key).await;

        // Assert
        assert_eq!(None, result);
    }

    #[tokio::test]
    async fn find_value_when_one_closest_and_chain_requests_it_should_return_final_value() {
        // Arrange
        let value = String::from("test value");
        let key = TestKey::new();
        let key_candidate_1 = TestKey::new();
        let key_candidate_2 = TestKey::new();
        let mut communicator = MockCommunicator::new();
        let mut routing_table = MockRoutingTable::new();
        routing_table.expect_store().times(1).return_const(());
        routing_table
            .expect_find_closest_nodes()
            .once()
            .return_const(vec![Node::new(key_candidate_1.clone(), TestLink::Link1)]);

        let k2 = key_candidate_2.clone();
        let return_value = value.clone();
        communicator
            .expect_find_value()
            .times(2)
            .returning(move |link, _| match link {
                TestLink::Link1 => Ok(FindValueResult::ClosestNodes(vec![Node::new(
                    k2.clone(),
                    TestLink::Link2,
                )])),
                TestLink::Link2 => Ok(FindValueResult::FoundValue(return_value.clone())),
                _ => Err(TestError::TestError),
            });

        // Act
        let result = kademlia_find_value(&communicator, &routing_table, &key).await;
        assert_eq!(Some(value), result);
    }
}
