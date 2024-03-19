use std::time::Duration;

use log::error;

use kademlia::{
    routing::SimpleRoutingTable,
    rpc::grpc::{GrpcCommunicator, KademliaGrpcInterfaceFactory},
    storage::HashMapKVS,
    DefaultKademliaParameters, KademliaNodeInstance, Key, Node,
};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    env_logger::init();
    // create node descriptor (id and link)
    let owner_id = Key::<DefaultKademliaParameters>::new();
    let owner = Node::new(owner_id.clone(), String::from("http://localhost:8080"));

    // create instance of communicator to reach other nodes
    let communicator = GrpcCommunicator::new(&owner);

    // create grpc interface for the node
    let node_interface_factory = KademliaGrpcInterfaceFactory::new("0.0.0.0:8080".into());

    // create in-memory storage
    let storage = HashMapKVS::<_, String>::new();

    // create routing table
    let routing_table = SimpleRoutingTable::new(&owner_id);

    // create node instance
    let node_instance =
        KademliaNodeInstance::new(node_interface_factory, communicator, storage, routing_table);

    // create cancellation token to stop instance by timeout
    let cancel = CancellationToken::new();
    let cloned_token = cancel.clone();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(10)).await;
        cloned_token.cancel();
    });

    // start server with a possibility to cancel it using the earlier created token
    let err = node_instance.start_with_cancel(cancel).await;
    let _ = err.inspect_err(|err| error!("error running instance: {}", err));
}
