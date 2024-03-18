mod key;

pub mod lookup;
pub mod node;
pub mod operations;
pub mod params;
pub mod routing;
pub mod rpc;
pub mod storage;

use std::marker::PhantomData;

use log::error;
pub use params::DefaultKademliaParameters;
pub use params::KademliaParameters;
pub use params::KeySizeParameters;

use routing::RoutingTable;
use rpc::KademliaServer;
pub use rpc::{Communicator, KademliaNodeInterface};
pub use storage::KeyValueStorage;

pub use key::Key;
pub use node::Node;

pub struct KademliaNodeInstance<
    P: KademliaParameters,
    Link: Clone,
    NodeInterface: KademliaServer<P, Link = Link>,
    C: Communicator<P, Link = Link>,
    KVS: KeyValueStorage<P>,
    RT: RoutingTable<P, Link = Link>,
> {
    node_interface: NodeInterface,
    storage: KVS,
    routing_table: RT,
    communicator: C,
    _phantom_p: PhantomData<P>,
}

impl<
        P: KademliaParameters,
        Link: Clone,
        N: KademliaServer<P, Link = Link>,
        C: Communicator<P, Link = Link>,
        KVS: KeyValueStorage<P>,
        R: RoutingTable<P, Link = Link, Communicator = C>,
    > KademliaNodeInstance<P, Link, N, C, KVS, R>
{
    pub fn new(node_interface: N, communicator: C, storage: KVS, routing_table: R) -> Self {
        Self {
            node_interface: node_interface,
            routing_table: routing_table,
            storage: storage,
            communicator: communicator,
            _phantom_p: PhantomData,
        }
    }

    pub async fn start(&self) -> Result<(), <N as KademliaServer<P>>::Error> {
        self.node_interface
            .start_server()
            .await
            .inspect_err(|err| error!("error while starting server: {}", err))
    }
}
