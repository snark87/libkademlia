mod key;

pub mod lookup;
pub mod node;
pub mod operations;
pub mod params;
pub mod routing;
pub mod rpc;
pub mod storage;

use std::marker::PhantomData;
use std::sync::Arc;

pub use params::DefaultKademliaParameters;
pub use params::KademliaParameters;
pub use params::KeySizeParameters;

use routing::RoutingTable;
use rpc::KademliaServer;
use rpc::KademliaServerFactory;
pub use rpc::{Communicator, KademliaNodeInterface};
pub use storage::KeyValueStorage;

pub use key::Key;
pub use node::Node;
use tokio_util::sync::CancellationToken;

pub struct KademliaNodeInstance<
    P: KademliaParameters,
    Link: Clone,
    Factory: KademliaServerFactory<P, Link = Link>,
    C: Communicator<P, Link = Link>,
    KVS: KeyValueStorage<P>,
    RT: RoutingTable<P, Link = Link>,
> {
    node_interface: Factory::Instance,
    storage: KVS,
    routing_table: Arc<RT>,
    communicator: C,
    _phantom_p: PhantomData<P>,
}

impl<
        P: KademliaParameters,
        Link: Clone + Send + Sync + 'static,
        N: KademliaServerFactory<P, Link = Link>,
        C: Communicator<P, Link = Link>,
        KVS: KeyValueStorage<P>,
        R: RoutingTable<P, Link = Link, Communicator = C>,
    > KademliaNodeInstance<P, Link, N, C, KVS, R>
{
    pub fn new(node_interface: N, communicator: C, storage: KVS, routing_table: R) -> Self {
        let routing_table = Arc::new(routing_table);
        let operations = operations::KademliaOperationsHandler::new(routing_table.clone());
        Self {
            node_interface: node_interface.new_server(operations),
            routing_table: routing_table,
            storage: storage,
            communicator: communicator,
            _phantom_p: PhantomData,
        }
    }

    pub async fn start(self) -> Result<(), <N::Instance as KademliaServer<P>>::Error> {
        let cancel = CancellationToken::new();
        self.start_with_cancel(cancel).await
    }

    pub async fn start_with_cancel(
        self,
        cancel: CancellationToken,
    ) -> Result<(), <N::Instance as KademliaServer<P>>::Error> {
        self.node_interface.start_server(cancel).await
    }
}
