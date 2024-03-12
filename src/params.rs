use generic_array::{typenum, ArrayLength};

/// Defines key size parameters for use within a system requiring specific key dimensions.
///
/// This trait is intended to be implemented by configurations that dictate
/// the size of keys within a given context. The `KeySize` associated type
/// is utilized to specify the exact dimensions of these keys, providing flexibility and safety by
/// leveraging Rust's type system to enforce key size constraints at compile time.
///
/// Implementors of this trait must provide a `KeySize` that satisfies the `ArrayLength` trait, ensuring
/// that the size can be associated with array types.
///
/// # Requirements
/// - `Clone`: Implementors of this trait must also implement `Clone` to allow for the trait object
///   to be cloned.
///
/// # Associated Types
/// - `KeySize`: The core of this trait, representing the dimension of keys managed or generated
///   by the implementing system. Must implement `ArrayLength` trait.
pub trait KeySizeParameters: Clone {
    type KeySize: ArrayLength;
}

/// Defines the core parameters for a Kademlia-based distributed hash table (DHT).
///
/// This trait extends `KeySizeParameters`, adding specific constants essential for
/// the operation of a Kademlia DHT. Implementors of this trait provide configuration
/// parameters that dictate the behavior and characteristics of the Kademlia network,
/// including key size through the inherited `KeySizeParameters` and operational parameters
/// defined within this trait.
///
/// Kademlia DHTs rely on these parameters for their routing and lookup operations, ensuring
/// efficient and scalable peer-to-peer networking. The parameters include `K_PARAM`, which
/// defines the maximum number of contacts stored in a bucket within the routing table,
/// and `ALPHA_PARAM`, which dictates the concurrency level of network requests during
/// node lookups and data storage/retrieval operations.
///
/// # Constants
/// - `K_PARAM`: The degree of redundancy and parallelism in the network. It specifies the maximum
///   number of contacts (nodes) in each k-bucket in the routing table. This parameter influences
///   the resilience and efficiency of the network, with higher values offering increased fault tolerance
///   at the cost of higher overhead.
///
/// - `ALPHA_PARAM`: Controls the number of parallel requests that the algorithm uses for network
///   operations such as node lookups.
///
/// # Usage
/// Implement this trait for any struct that will act as a configuration container for Kademlia-based
/// systems.
///
/// # Examples
/// ```
/// use kademlia::{KeySizeParameters, KademliaParameters};
/// use generic_array::typenum::*;
///
/// #[derive(Clone)]
/// struct MyKademliaConfig;
///
/// impl KeySizeParameters for MyKademliaConfig {
///     type KeySize = U20; //160-bit key
/// }
///
/// impl KademliaParameters for MyKademliaConfig {
///     const K_PARAM: u8 = 20;
///     const ALPHA_PARAM: u8 = 3;
/// }
/// ```
///
/// This trait ensures that the fundamental parameters necessary for the functioning and optimization
/// of Kademlia DHTs are explicitly defined and accessible, promoting clear, consistent, and configurable
/// implementations of the protocol.
pub trait KademliaParameters: KeySizeParameters {
    const K_PARAM: u8;
    const ALPHA_PARAM: u8;
}

/// Defines default set of Kademlia parameters with `k`=20, `alpha`=3, and 160-byte keys
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DefaultKademliaParameters;

impl KademliaParameters for DefaultKademliaParameters {
    const K_PARAM: u8 = 20;
    const ALPHA_PARAM: u8 = 3;
}

impl KeySizeParameters for DefaultKademliaParameters {
    type KeySize = typenum::U20;
}
