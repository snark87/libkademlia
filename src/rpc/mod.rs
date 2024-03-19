use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::{key::Key, node::Node, operations, params::KeySizeParameters, KademliaParameters};

pub mod grpc;

/// Represents the outcome of a `find_value` operation in a Kademlia Distributed Hash Table (DHT).
///
/// This enum has two variants:
///
/// - `ClosestNodes(Vec<Node<P, Link>>)` is returned when the value associated with the requested key is not found.
///   Instead, it provides a vector of the closest nodes to the requested key, based on the XOR metric. These nodes are
///   potential candidates for either storing the sought value or knowing where it might be located.
///
/// - `FoundValue(V)` is returned when the exact value associated with the requested key is found within the node.
///   `V` is the type of the found value, directly providing what was requested.
#[derive(Debug)]
pub enum FindValueResult<P: KeySizeParameters, Link: Clone, V> {
    ClosestNodes(Vec<Node<P, Link>>),
    FoundValue(V),
}

#[async_trait]
pub trait NodeAvailabilityChecker {
    type Error: std::error::Error;
    type Link: Clone + Sync + Send;

    /// Asynchronously sends a ping to another node in the network to verify its availability.
    ///
    /// This method helps maintain the network's health by identifying active versus inactive nodes.
    ///
    /// Parameters:
    /// - `link`: A reference to the `Link` associated with the node to be pinged.
    ///
    /// Returns:
    /// - `Ok(())` if the ping is successful and the node is reachable.
    /// - `Err(Self::Error)` detailing the error encountered, indicating the node might be down or unreachable.
    async fn ping(&self, link: &Self::Link) -> Result<(), Self::Error>;
}

/// Defines the communication capabilities required for a node within a Kademlia Distributed Hash Table (DHT) network.
///
/// This trait encapsulates the essential network operations that a Kademlia node must be able to perform, including
/// finding the closest nodes to a given key, sending pings to other nodes, finding a value associated with a key, and
/// storing a value in the network. It is designed to work asynchronously to accommodate the non-blocking nature of
/// network I/O operations.
///
/// Type Parameters:
/// - `P`: Conforms to the `KeySizeParameters` trait, defining the size of keys used within the DHT.
///
/// Associated Types:
/// - `Link`: Represents a connection or reference to another node in the network. It must implement `Clone` to allow
///   for efficient duplication.
/// - `Error`: Defines the error type for operations, which must implement the `std::error::Error` trait.
/// - `Value`: Represents the type of values that can be stored and retrieved from the DHT.
///
/// Required Methods:
/// - `async fn get_k_closest`: Asynchronously retrieves a vector of the `k` closest nodes to a given key (`FIND_NODE` in Kademlia paper).
/// - `async fn ping`: Asynchronously sends a ping to another node, used to check if it is alive (`PING` in Kademlia paper).
/// - `async fn find_value`: Asynchronously attempts to find the value associated with a given key in the DHT. Returns
///   either the value or the closest nodes to the key if the value is not found (`FIND_VALUE` in Kademlia paper).
/// - `async fn store_value`: Asynchronously stores a value in the network, making it accessible to other nodes (`STORE` in Kademlia paper).
///
/// This trait must be implemented by any entity that wishes to participate in the Kademlia DHT as a fully functional
/// node, capable of both responding to requests from others and initiating its own queries within the network.
#[async_trait]
pub trait Communicator<P: KeySizeParameters>:
    NodeAvailabilityChecker + Send + Sync + 'static
{
    type Value;

    /// Asynchronously retrieves a list of the `k` closest nodes to a given key within the DHT network.
    ///
    /// This method is crucial for locating nodes that are likely to store or know the location of a particular value.
    ///
    /// Parameters:
    /// - `link`: A reference to the `Link` associated with the node initiating the request.
    /// - `key`: The key for which the closest nodes are being sought.
    ///
    /// Returns:
    /// - `Ok(Vec<Node<P, Self::Link>>)` containing the closest nodes if successful.
    /// - `Err(Self::Error)` detailing the error encountered during the operation.
    async fn get_k_closest(
        &self,
        link: &Self::Link,
        key: &Key<P>,
    ) -> Result<Vec<Node<P, Self::Link>>, Self::Error>;

    /// Asynchronously attempts to find the value associated with a given key in the DHT network.
    ///
    /// This method either returns the value directly if it's found, or the closest nodes to the key, aiding in the
    /// search process.
    ///
    /// Parameters:
    /// - `link`: A reference to the `Link` associated with the node initiating the request.
    /// - `key`: The key associated with the value being sought.
    ///
    /// Returns:
    /// - `Ok(FindValueResult<P, Self::Link, Self::Value>)` with the result of the search, which could be the value
    ///   itself or the closest nodes.
    /// - `Err(Self::Error)` detailing the error encountered during the search.
    async fn find_value(
        &self,
        link: &Self::Link,
        key: &Key<P>,
    ) -> Result<FindValueResult<P, Self::Link, Self::Value>, Self::Error>;

    /// Asynchronously stores a value in the network at the node closest to the value's key.
    ///
    /// This method is fundamental for distributing and storing data across the DHT network, ensuring data availability
    /// and redundancy.
    ///
    /// Parameters:
    /// - `link`: A reference to the `Link` associated with the node where the value is to be stored.
    /// - `key`: A reference to key to store the value at.
    /// - `value`: The value to be stored in the network.
    ///
    /// Returns:
    /// - `Ok(())` if the storage operation is successful.
    /// - `Err(Self::Error)` detailing any issues encountered during the storage operation.
    async fn store_value(
        &self,
        link: &Self::Link,
        key: &Key<P>,
        value: Self::Value,
    ) -> Result<(), Self::Error>;
}

pub trait KademliaServerFactory<P: KademliaParameters> {
    type Link: Clone;
    type Instance: KademliaServer<P, Link = Self::Link>;

    fn new_server(
        &self,
        operations: impl operations::KademliaOperations<P, Link = Self::Link> + 'static,
    ) -> Self::Instance;
}

#[async_trait]
pub trait KademliaServer<P: KademliaParameters> {
    type Error: std::error::Error;
    type Link: Clone;

    async fn start_server(self, cancel: CancellationToken) -> Result<(), Self::Error>;

    fn get_link(&self) -> &Self::Link;
}

pub trait KademliaNodeInterface<P: KademliaParameters, Link: Clone>:
    Communicator<P, Link = Link> + KademliaServer<P, Link = Link> + Send
{
}
